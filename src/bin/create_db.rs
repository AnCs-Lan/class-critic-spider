use sqlx::SqlitePool;
use std::str::FromStr;

#[tokio::main]
async fn main() {
    // 加载环境变量
    dotenvy::dotenv().ok();

    let database_url = std::env::var("DATABASE_URL").expect("请在 .env 文件中设置 DATABASE_URL");

    // 建立连接
    let connect_options = sqlx::sqlite::SqliteConnectOptions::from_str(&database_url)
        .expect("数据库URL解析失败")
        .create_if_missing(true);

    let pool = SqlitePool::connect_with(connect_options)
        .await
        .expect("数据库连接失败");

    println!("正在初始化数据库表...");

    // 执行建表语句
    // 注意：这里必须用 query() 而不是 query!()，避免编译时检查
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

        CREATE INDEX IF NOT EXISTS idx_review_teacher_course
        ON review(teacher_id, course_id);
        "#,
    )
    .execute(&pool)
    .await
    .expect("建表失败");

    println!("数据库表创建/验证成功！");
}
