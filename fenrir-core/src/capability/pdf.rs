use std::path::Path;

use anyhow::{Context, Result, bail};

use crate::files::operations::{
  create_directory_all, file_exists, get_file_size, validate_file_exists,
};
use crate::utils::image::optimize_image;
use crate::utils::process;
use crate::utils::xdg;

const RENDER_DPI: &str = "100";

pub struct PdfInfo {
  pub pages: u32,
  pub file_size: String,
}

pub async fn pdf_info(pdf: &str) -> Result<PdfInfo> {
  validate_file_exists(pdf).await?;

  let pages = pdf_page_count(pdf).await?;
  let file_size = file_size_label(pdf).await?;

  return Ok(PdfInfo { pages, file_size });
}

pub async fn render_page(pdf: &str, page: u32) -> Result<String> {
  validate_file_exists(pdf).await?;
  if page == 0 {
    bail!("Page numbers are 1-based");
  }

  let output_dir = output_dir().await?;
  let prefix = image_prefix(pdf);
  let output_path = format!("{output_dir}/{prefix}");
  let page_str = page.to_string();

  let output = process::run(
    "pdftoppm",
    &[
      "-png",
      "-f",
      page_str.as_str(),
      "-l",
      page_str.as_str(),
      "-r",
      RENDER_DPI,
      pdf,
      &output_path,
    ],
  )
  .await
  .context(format!("Failed to run pdftoppm for '{pdf}'"))?;

  if !output.exit_code.is_success() {
    let stderr = output.stderr.trim();
    let details = if stderr.is_empty() {
      format!("pdftoppm could not render page {page} of '{pdf}'")
    } else {
      stderr.to_string()
    };
    bail!("{details}");
  }

  let png_path = format!("{output_path}-{page}.png");
  if !file_exists(&png_path).await {
    bail!("Page {page} of '{pdf}' does not exist");
  }

  return Ok(optimize_image(&png_path).await);
}

async fn output_dir() -> Result<String> {
  let data_home =
    xdg::data_home().context("Cannot determine the fenrir data directory")?;
  let dir = data_home.join("pdf_images").to_string_lossy().to_string();
  create_directory_all(&dir).await?;
  return Ok(dir);
}

fn image_prefix(pdf: &str) -> String {
  let stem = Path::new(pdf)
    .file_stem()
    .map(|stem| stem.to_string_lossy().to_string())
    .unwrap_or_else(|| "pdf".to_string());
  let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
  return format!("{stem}_{timestamp}");
}

async fn pdf_page_count(pdf: &str) -> Result<u32> {
  let output = process::run("pdfinfo", &[pdf])
    .await
    .context(format!("Failed to run pdfinfo for '{pdf}'"))?;
  if !output.exit_code.is_success() {
    let stderr = output.stderr.trim();
    let details = if stderr.is_empty() {
      format!("pdfinfo could not inspect '{pdf}'")
    } else {
      stderr.to_string()
    };
    bail!("{details}");
  }
  for line in output.stdout.lines() {
    if let Some(value) = line.strip_prefix("Pages:") {
      let pages = value.trim().parse::<u32>().context(format!(
        "Could not parse page count from pdfinfo output for '{pdf}'"
      ))?;
      if pages == 0 {
        bail!("pdfinfo reported 0 pages for '{pdf}'");
      }
      return Ok(pages);
    }
  }
  bail!("pdfinfo produced no page count for '{pdf}'");
}

async fn file_size_label(pdf: &str) -> Result<String> {
  let bytes = get_file_size(pdf)
    .await
    .context(format!("Cannot determine the size of '{pdf}'"))?;
  if bytes < 1024 * 1024 {
    return Ok(format!("{:.0} KB", bytes as f64 / 1024.0));
  }
  return Ok(format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0)));
}
