mod types;
use crate::types::{LeanCloudResponse, Review};
use chrono::{SecondsFormat, Utc};
use reqwest::header::{CONTENT_TYPE, HeaderMap, HeaderValue, USER_AGENT};
use std::str::FromStr;
use std::time::Duration;
use tokio::sync::mpsc;

fn get_client() -> reqwest::Client {
    let mut headers = HeaderMap::new();
    let user_agent = std::env::var("USER_AGENT").expect("env文件中没有变量user_agent");
    let content_type = std::env::var("CONTENT_TYPE").expect("env文件中没有变量CONTENT_TYPE");
    let origin = std::env::var("Origin").expect("env文件中没有变量Origin");
    let referer = std::env::var("Referer").expect("env文件中没有变量Referer");
    let x_lc_id = std::env::var("X_LC_Id").expect("env文件中没有变量X_LC_Id");
    let x_lc_session = std::env::var("X_LC_Session").expect("env文件中没有变量X_LC_Session");
    let x_lc_ua = std::env::var("X_LC_UA").expect("env文件中没有变量X_LC_UA");
    let x_lc_sign = std::env::var("X_LC_Sign").expect("env文件中没有变量X_LC_Sign");
    headers.insert(USER_AGENT, HeaderValue::from_str(&user_agent).unwrap());
    headers.insert(CONTENT_TYPE, HeaderValue::from_str(&content_type).unwrap());
    headers.insert("Origin", HeaderValue::from_str(&origin).unwrap());
    headers.insert("Referer", HeaderValue::from_str(&referer).unwrap());
    headers.insert("X-LC-Id", HeaderValue::from_str(&x_lc_id).unwrap());
    headers.insert("X-LC-UA", HeaderValue::from_str(&x_lc_ua).unwrap());
    headers.insert(
        "X-LC-Session",
        HeaderValue::from_str(&x_lc_session).unwrap(),
    );
    headers.insert("X-LC-Sign", HeaderValue::from_str(&x_lc_sign).unwrap());
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

            let course_data = match item.course_data.clone() {
                Some(data) => data,
                None => {
                    println!(
                        "跳过了一条脏数据：评价 ID 为 {} 的记录缺失 courseId 字段",
                        item.review_id
                    );
                    continue;
                }
            };

            let author_data = match item.author_data.clone() {
                Some(data) => data,
                None => {
                    println!("跳过脏数据: 评价 {} 缺失 authorId", item.review_id);
                    continue;
                }
            };

            let teacher_row = sqlx::query!(
                r#"
                INSERT INTO teacher (name, position, dept) VALUES (?, ?, ?)
                ON CONFLICT(name, dept) DO UPDATE SET position=excluded.position
                RETURNING id
                "#,
                &item.teacher_name,
                &course_data.position,
                &course_data.dept
            )
            .fetch_one(&mut *tx)
            .await
            .unwrap();
            let local_teacher_id: i64 = teacher_row.id;

            let course_row = sqlx::query!(
                r#"
                INSERT INTO course (origin_id,name) VALUES (?,?)
                ON CONFLICT(origin_id) DO UPDATE SET name=excluded.name
                RETURNING id
                "#,
                &course_data.object_id,
                &item.course_name
            )
            .fetch_one(&mut *tx)
            .await
            .unwrap();
            let local_course_id: i64 = course_row.id;
            let flat_tags = item.extract_all_tags();
            let tags_json = serde_json::to_string(&flat_tags).unwrap_or_else(|_| "[]".to_string());

            sqlx::query!(
                r#"
                INSERT OR IGNORE INTO review (id, teacher_id, course_id, user_name, comment, pro, exp, frd, total, up_vote, down_vote, tags, time)
                VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?);
                "#,
                &item.review_id,
                local_teacher_id,
                local_course_id,
                &author_data.user_name,
                &item.comment,
                item.rating.pro,
                item.rating.exp,
                item.rating.frd,
                item.rating.total,
                item.up_vote,
                item.down_vote,
                tags_json,
                &item.time
            )
            .execute(&mut *tx)
            .await
            .unwrap();

            println!("已写入一段数据");
        }
        tx.commit().await.unwrap();
    }
}

async fn network_producer(tx: mpsc::Sender<Vec<Review>>, latest_time: Option<String>) {
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

        tx.send(reviews.clone()).await.unwrap();

        if let Some(last_one) = reviews.last() {
            current_time = last_one.time.clone();
            if let Some(db_time) = latest_time
                .clone()
                .filter(|t| current_time.as_str() < t.as_str())
            {
                println!("数据获取完成，截止时间{}", db_time);
                break;
            }
        }

        tokio::time::sleep(Duration::from_millis(1000)).await;
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
        CREATE TABLE IF NOT EXISTS teacher (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            position TEXT,
            dept TEXT,
            UNIQUE(name,dept)
        );

        CREATE TABLE IF NOT EXISTS course (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            origin_id TEXT NOT NULL UNIQUE,
            name TEXT NOT NULL
        );
        
        CREATE TABLE IF NOT EXISTS review (
            id TEXT PRIMARY KEY,
            teacher_id INTEGER,
            course_id INTEGER,
            user_name TEXT,
            comment TEXT NOT NULL,
            pro INTEGER,
            exp INTEGER,
            frd INTEGER,
            total INTEGER,
            up_vote INTEGER DEFAULT 1,
            down_vote INTEGER DEFAULT 1,
            tags TEXT,
            time TEXT NOT NULL,
            FOREIGN KEY(teacher_id) REFERENCES teacher(id),
            FOREIGN KEY(course_id) REFERENCES course(id)
        );

        -- 联合查询优化
        CREATE INDEX IF NOT EXISTS idx_review_teacher_course
        ON review(teacher_id, course_id);
        "#,
    )
    .execute(&pool)
    .await?;
    println!("数据库检查完毕");

    let latest_time: Option<String> =
        sqlx::query_scalar("SELECT time FROM review ORDER BY time DESC LIMIT 1")
            .fetch_optional(&pool)
            .await?
            .flatten();

    let (tx, rx) = mpsc::channel::<Vec<Review>>(100);
    let worker_handel = tokio::spawn(async move {
        database_worker(rx, pool.clone()).await;
    });
    network_producer(tx, latest_time).await;
    worker_handel.await?;
    Ok(())
}
