use crate::files::operations::remove_file;
use crate::utils::process;

pub async fn optimize_image(path: &str) -> String {
  let jpeg_path = jpeg_path_for(path);
  let output = process::run(
    "magick",
    &[
      path,
      "-background",
      "white",
      "-alpha",
      "remove",
      "-quality",
      "85",
      jpeg_path.as_str(),
    ],
  )
  .await;
  if let Ok(output) = output
    && output.exit_code.is_success()
  {
    let _ = remove_file(path).await;
    return jpeg_path;
  }
  let _ = process::run("optipng", &["-quiet", "-o2", path]).await;
  return path.to_string();
}

fn jpeg_path_for(path: &str) -> String {
  return match path.rsplit_once('.') {
    Some((stem, _)) => format!("{stem}.jpg"),
    None => format!("{path}.jpg"),
  };
}
