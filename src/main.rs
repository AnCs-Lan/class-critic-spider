use chrono::{SecondsFormat, Utc};
use reqwest::header::{CONTENT_TYPE, HeaderMap, HeaderValue, USER_AGENT};
use serde::Deserialize;
use std::str::FromStr;
use std::time::Duration;
use tokio::sync::mpsc;

#[derive(Deserialize, Default)]
struct Ratings {
    #[serde(rename = "rate1", default)]
    professionalism: u8,
    #[serde(rename = "rate2", default)]
    expressive: u8,
    #[serde(rename = "rate3", default)]
    friendliness: u8,
    #[serde(rename = "overall", default)]
    total: u8,
}

#[derive(Deserialize, Default)]
struct Review {
    #[serde(rename = "objectId", default)]
    review_id: String,
    #[serde(rename = "profName", default)]
    teacher_name: String,
    #[serde(rename = "courseName", default)]
    course_name: String,
    #[serde(rename = "createdAt", default)]
    create_date: String,
    #[serde(rename = "upVote", default)]
    up_vote: u32,
    #[serde(rename = "downVote", default)]
    down_vote: u32,
    #[serde(rename = "comment", default)]
    comment: String,
    #[serde(rename = "rating", default)]
    ratings: Ratings,
}

#[derive(Deserialize)]
struct LeanCloudResponse {
    #[serde(default)]
    results: Vec<Review>,
}

fn get_client() -> reqwest::Client {
    let mut headers = HeaderMap::new();
    headers.insert(
        USER_AGENT,
        HeaderValue::from_static(
            "Mozilla/5.0 (X11; Linux x86_64; rv:151.0) Gecko/20100101 Firefox/151.0",
        ),
    );
    headers.insert(
        CONTENT_TYPE,
        HeaderValue::from_static("application/json;charset=UTF-8"),
    );
    headers.insert(
        "Origin",
        HeaderValue::from_static("https://app.huoshui.org"),
    );
    headers.insert(
        "Referer",
        HeaderValue::from_static("https://app.huoshui.org/"),
    );
    headers.insert(
        "X-LC-Id",
        HeaderValue::from_static("zwjjm3MbxDYRKny9f31amkXq"),
    );
    headers.insert(
        "X-LC-UA",
        HeaderValue::from_static("LeanCloud-JS-SDK/4.14.0 (Browser)"),
    );
    headers.insert(
        "X-LC-Sign",
        HeaderValue::from_static("632cbdae5e35771de22b88b0a690366b,1781851480568"),
    );
    reqwest::Client::builder()
        .default_headers(headers)
        .build()
        .unwrap()
}

async fn database_worker(mut rx: mpsc::Receiver<Vec<Review>>, pool: sqlx::SqlitePool) {
    while let Some(batch_data) = rx.recv().await {
        let mut tx = pool.begin().await.unwrap();
        for item in batch_data {
            if item.review_id.is_empty() {
                continue;
            }

            let teacher_row = sqlx::query!(
                r#"
                INSERT INTO teachers (name) VALUES (?)
                ON CONFLICT(name) DO UPDATE SET name=name
                RETURNING id
                "#,
                &item.teacher_name
            )
            .fetch_one(&mut *tx)
            .await
            .unwrap();
            let local_teacher_id: i64 = teacher_row.id;

            let course_row = sqlx::query!(
                r#"
                INSERT INTO courses (name) VALUES (?)
                ON CONFLICT(name) DO UPDATE SET name=name
                RETURNING id
                "#,
                &item.course_name
            )
            .fetch_one(&mut *tx)
            .await
            .unwrap();
            let local_course_id: i64 = course_row.id;

            sqlx::query!(
                r#"
                INSERT OR IGNORE INTO reviews(id, teacher_id, course_id, comment, rate_professinalism, rate_expressive, rate_friendliness, rate_total, up_vote, down_vote)
                VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?);
                "#,
                &item.review_id,
                local_teacher_id,
                local_course_id,
                &item.comment,
                item.ratings.professionalism,
                item.ratings.expressive,
                item.ratings.friendliness,
                item.ratings.total,
                item.up_vote,
                item.down_vote,
            )
            .execute(&mut *tx)
            .await
            .unwrap();

            println!("已写入一段数据");
        }
        tx.commit().await.unwrap();
    }
}

async fn network_producer(tx: mpsc::Sender<Vec<Review>>) {
    let client = get_client();
    let mut current_time = Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true);
    loop {
        let url = format!(
            r#"https://lean.huoshui.org/1.1/classes/Reviews?where={{"createdAt":{{"$lt":{{"__type":"Date","iso":"{}"}}}}}}&include=authorId,courseId&limit=50&order=-createdAt"#,
            current_time
        );

        let res = client.get(&url).send().await.unwrap();
        let data: LeanCloudResponse = res.json().await.unwrap();
        let reviews = data.results;

        if reviews.is_empty() {
            println!("数据获取完毕");
            break;
        }

        if let Some(last_one) = reviews.last() {
            current_time = last_one.create_date.clone();
        }

        tx.send(reviews).await.unwrap();
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();
    let database_url = std::env::var("DATABASE_URL").expect("env文件中没有变量DATABASE_URL");
    let connect_options =
        sqlx::sqlite::SqliteConnectOptions::from_str(&database_url)?.create_if_missing(true);
    let pool = sqlx::SqlitePool::connect_with(connect_options).await?;
    println!("已连接数据库");

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS teachers (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL UNIQUE
        );

        CREATE TABLE IF NOT EXISTS courses (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL UNIQUE
        );
        
        CREATE TABLE IF NOT EXISTS reviews (
            id TEXT PRIMARY KEY,
            teacher_id INTEGER,
            course_id INTEGER,
            comment TEXT NOT NULL,
            rate_professinalism INTEGER,
            rate_expressive INTEGER,
            rate_friendliness INTEGER,
            rate_total INTEGER,
            up_vote INTEGER,
            down_vote INTEGER,
            FOREIGN KEY(teacher_id) REFERENCES teachers(id),
            FOREIGN KEY(course_id) REFERENCES courses(id)
        )
        "#,
    )
    .execute(&pool)
    .await?;
    println!("数据库检查完毕");

    let (tx, rx) = mpsc::channel::<Vec<Review>>(100);
    tokio::spawn(async move {
        database_worker(rx, pool.clone()).await;
    });
    network_producer(tx).await;

    Ok(())
}
