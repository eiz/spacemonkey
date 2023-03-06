use std::{
    collections::HashMap,
    fs::{self},
    path::{Path, PathBuf},
};

use futures_util::StreamExt;
use serde::{Deserialize, Serialize};

use crate::openai;

async fn generate_one_topic(
    openai_key: String,
    topic: &str,
    out_path: PathBuf,
) -> anyhow::Result<()> {
    let text = openai::gpt_basic_data(
        openai_key,
        "You are a helpful assistant.",
        format!(
            r#"Write a list of 50 simple questions and answers about {}. Answer with this format:

Q: the question here
A: the answer here
Q: another question
A: another answer"#,
            topic
        ),
    )
    .await?;
    fs::write(out_path, text)?;
    Ok(())
}

pub async fn extract_topic_questions<P: AsRef<Path>, Q: AsRef<Path>>(
    openai_key: String,
    path: P,
    out_dir: Q,
) -> anyhow::Result<()> {
    let out_dir = out_dir.as_ref();
    fs::create_dir_all(out_dir)?;
    let text = fs::read_to_string(path.as_ref())?;
    let lines = text.lines().collect::<Vec<_>>();

    let _ = tokio_stream::iter(lines.into_iter().map(move |el| {
        let openai_key = openai_key.clone();
        let out_path = out_dir.join(format!("{}.txt", el)).to_path_buf();
        generate_one_topic(openai_key, el, out_path)
    }))
    .buffer_unordered(8)
    .collect::<Vec<anyhow::Result<()>>>()
    .await;
    Ok(())
    //
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct QuestionPair {
    question: String,
    answer: String,
}

pub fn dedup<P: AsRef<Path>, Q: AsRef<Path>>(in_dir: P, out_file: Q) -> anyhow::Result<()> {
    let in_dir = in_dir.as_ref();
    let out_file = out_file.as_ref();
    let mut questions = HashMap::new();

    for child in fs::read_dir(in_dir)? {
        let child = child?;
        let text = fs::read_to_string(child.path())?;
        let mut question = None;

        for line in text.lines() {
            if line.starts_with("Q: ") {
                question = Some(line[3..].to_owned());
            }

            if question.is_some() && line.starts_with("A: ") {
                questions.insert(question.take().unwrap(), line[3..].to_owned());
            }
        }
    }

    let questions = questions
        .into_iter()
        .map(|(question, answer)| QuestionPair { question, answer })
        .collect::<Vec<_>>();
    fs::write(out_file, serde_json::to_string(&questions)?)?;
    Ok(())
}
