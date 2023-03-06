use std::{
    fs,
    path::{Path, PathBuf},
};

use futures_util::StreamExt;

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
