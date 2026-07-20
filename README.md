# 交大活水数据库构建

## 基本工作

### 运行前

由于sqlx的query!宏限制，在获取数据前必须拥有一个符号要求的数据库
运行:

```bash
cargo run --bin create_db
```

以构建一个表

### 构建环境

需要在项目根目录创建环境`.env`文件并填补字段：

```.env
DATABASE_URL="sqlite:./reviews.db"
USER_AGENT=""
CONTENT_TYPE="application/json;charset=UTF-8"
Origin="https://app.huoshui.org"
Referer="https://app.huoshui.org/"
X_LC_Id=""
X_LC_Session=""
X_LC_UA=""
X_LC_Sign=""
```

这将用于网络请求正确运行

### 启动项目

然后需要再把运行目标定向到原始项目运行：

```bash
cargo run --bin class-critic-spider
```

## 项目特点

### 评论标签数据合并

由于活水的json数据排布较为凌乱，此项目对于出现可能性不稳定的各类标签做了**合并处理**  
具体为:

- 课堂氛围
- 作业量
- 水课鉴定
- 签到点名
- 考试体验：开卷、划重点、原题、给分宽松  
  这几个标签被同一合并为了一个`json`数组（仅储存单评论中出现的标签）

### 教师区分

由于**同名教师**的存在以及活水json数据中不存在对应的**身份ID**  
此数据库的存储采用以（姓名，院系）作为联合区分方案

### 数据更新

数据库内存储了每一条评论的时间，并已被用于更新数据库，每次运行项目将**仅更新到上一次最新时间**而非全面获取  
除非数据库是新建的

## 数据库表格构成

```sql
        teacher (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            position TEXT,
            dept TEXT,
            UNIQUE(name,dept)
        );

        course (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            origin_id TEXT NOT NULL UNIQUE,
            name TEXT NOT NULL
        );

        review (
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
        idx_review_teacher_course
        ON review(teacher_id, course_id);
```
