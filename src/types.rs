use serde::Deserialize;

#[derive(Deserialize, Clone, Default, Debug)]
pub struct Ratings {
    #[serde(rename = "rate1", default)]
    pub pro: i32,
    #[serde(rename = "rate2", default)]
    pub exp: i32,
    #[serde(rename = "rate3", default)]
    pub frd: i32,
    #[serde(rename = "overall", default)]
    pub total: i32,
}

#[derive(Deserialize, Clone, Default, Debug)]
pub struct CourseIdObj {
    #[serde(rename = "objectId")]
    pub object_id: String,
    #[serde(rename = "position", default)]
    pub position: String,
    #[serde(rename = "dept", default)]
    pub dept: String,
}

#[derive(Deserialize, Clone, Default, Debug)]
pub struct AuthorIdObj {
    #[serde(rename = "username", default)]
    pub user_name: String,
}

#[derive(Deserialize, Clone, Default, Debug)]
pub struct CustomTagItem {
    #[serde(default)]
    pub value: String,
    #[serde(default)]
    pub checked: bool,
}

//考勤/作业/水课样式
#[derive(Deserialize, Clone, Default, Debug)]
pub struct MetricItem {
    #[serde(default)]
    pub name: String, //程度标签
}

#[derive(Deserialize, Clone, Default, Debug)]
pub struct ExamTagItem {
    #[serde(default)]
    pub checked: bool,
}

#[derive(Deserialize, Clone, Default, Debug)]
pub struct Exam {
    #[serde(default)]
    pub examprep: Option<ExamTagItem>, //划重点
    #[serde(default)]
    pub openbook: Option<ExamTagItem>, //开卷
    #[serde(default)]
    pub oldquestion: Option<ExamTagItem>, //原题
    #[serde(default)]
    pub easiness: Option<ExamTagItem>, //给分
}

#[derive(Deserialize, Clone, Default)]
pub struct Review {
    #[serde(rename = "objectId")]
    pub review_id: String,
    #[serde(rename = "profName", default)]
    pub teacher_name: String,
    #[serde(rename = "courseName", default)]
    pub course_name: String,
    #[serde(rename = "createdAt", default)]
    pub time: String,
    #[serde(rename = "upVote", default)]
    pub up_vote: i32,
    #[serde(rename = "downVote", default)]
    pub down_vote: i32,
    #[serde(default)]
    pub comment: String,
    #[serde(default)]
    pub rating: Ratings,
    #[serde(rename = "courseId")]
    pub course_data: Option<CourseIdObj>,
    #[serde(rename = "authorId")]
    pub author_data: Option<AuthorIdObj>,

    pub tags: Option<Vec<CustomTagItem>>,
    pub attendance: Option<MetricItem>,
    pub bird: Option<MetricItem>,
    pub homework: Option<MetricItem>,
    pub exam: Option<Exam>,
}

impl Review {
    pub fn extract_all_tags(&self) -> Vec<String> {
        let mut all_tags = Vec::new();
        let is_valid_tag = |s: &str| -> bool {
            let trimmed = s.trim();
            !trimmed.is_empty() && trimmed != "未填" && trimmed != "未选择" && trimmed != "暂无"
        };
        // 氛围tags
        if let Some(ref custom_tags) = self.tags {
            for tag in custom_tags {
                if tag.checked && is_valid_tag(&tag.value) {
                    all_tags.push(tag.value.trim().to_string());
                }
            }
        }
        // 点名
        if let Some(att) = self.attendance.as_ref().filter(|a| is_valid_tag(&a.name)) {
            all_tags.push(format!("点名情况:{}", att.name.trim()));
        }
        // 水课鉴定
        if let Some(b) = self.bird.as_ref().filter(|b| is_valid_tag(&b.name)) {
            all_tags.push(b.name.trim().to_string());
        }
        // 作业量
        if let Some(hw) = self.homework.as_ref().filter(|h| is_valid_tag(&h.name)) {
            all_tags.push(format!("作业量:{}", hw.name.trim()));
        }
        // 考试
        if let Some(ref ex) = self.exam {
            if let Some(true) = ex.openbook.as_ref().map(|item| item.checked) {
                all_tags.push("开卷".to_string());
            }
            if let Some(true) = ex.examprep.as_ref().map(|item| item.checked) {
                all_tags.push("划重点".to_string());
            }
            if let Some(true) = ex.oldquestion.as_ref().map(|item| item.checked) {
                all_tags.push("做过的原题较多".to_string());
            }
            if let Some(true) = ex.easiness.as_ref().map(|item| item.checked) {
                all_tags.push("给分比较宽松".to_string());
            }
        }

        all_tags
    }
}

#[derive(Deserialize)]
pub struct LeanCloudResponse {
    #[serde(default)]
    pub results: Vec<Review>,
}
