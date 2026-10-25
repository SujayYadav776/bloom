// Wallpaper changer: enumerates desktop wallpapers from several common Windows
// locations, renders small base64 thumbnails for the notch preview strip, and
// sets the active wallpaper via the Win32 SystemParametersInfoW API.
//
// Thumbnails are returned as `data:image/jpeg;base64,...` URLs so they render
// under the app's existing CSP (`img-src 'self' data: blob:`) without needing
// the asset protocol or capability changes.

use std::ffi::c_void;
use std::os::windows::ffi::OsStrExt;
use std::path::{Path, PathBuf};

use base64::Engine;
use image::imageops::FilterType;
use image::{GenericImageView, ImageFormat};
use serde::Serialize;
use tauri::AppHandle;
use windows::Win32::UI::WindowsAndMessaging::{
	SystemParametersInfoW, SPIF_SENDCHANGE, SPIF_UPDATEINIFILE, SPI_GETDESKWALLPAPER,
	SPI_SETDESKWALLPAPER, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS,
};

/// A single wallpaper entry with a small inline thumbnail for the preview strip.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WallpaperThumb {
	pub name: String,
	pub path: String,
	pub data_url: String,
}

const THUMB_WIDTH: u32 = 320;
const MAX_RESULTS: usize = 36;

fn is_image(path: &Path) -> bool {
	matches!(
		path.extension().and_then(|e| e.to_str()).map(|e| e.to_ascii_lowercase()).as_deref(),
		Some("jpg") | Some("jpeg") | Some("png") | Some("bmp")
	)
}

fn name_from_path(path: &Path) -> String {
	path.file_stem()
		.and_then(|s| s.to_str())
		.unwrap_or("Wallpaper")
		.to_string()
}

/// Collect candidate wallpaper files from the standard Windows locations plus an
/// optional user folder (setting `bloom-wallpaper-folder`). Bounded by depth and
/// total count so the panel stays responsive.
fn collect_wallpapers(app: &AppHandle) -> Vec<PathBuf> {
	let mut roots: Vec<PathBuf> = Vec::new();

	let system_root = std::env::var("SystemRoot").unwrap_or_else(|_| "C:\\Windows".into());
	roots.push(PathBuf::from(&system_root).join("Web").join("Wallpapers"));
	roots.push(PathBuf::from(&system_root).join("Web").join("Screen"));

	if let Ok(profile) = std::env::var("USERPROFILE") {
		roots.push(PathBuf::from(profile).join("Pictures"));
	}
	if let Ok(public) = std::env::var("PUBLIC") {
		roots.push(PathBuf::from(public).join("Pictures"));
	}
	if let Some(custom) = crate::utils::get_setting_str(app, "bloom-wallpaper-folder") {
		if !custom.trim().is_empty() {
			roots.push(PathBuf::from(custom.trim()));
		}
	}

	let mut out: Vec<PathBuf> = Vec::new();
	let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();

	for root in roots {
		if out.len() >= MAX_RESULTS {
			break;
		}
		walk(&root, 0, &mut out, &mut seen);
	}
	out
}

fn walk(dir: &Path, depth: usize, out: &mut Vec<PathBuf>, seen: &mut std::collections::HashSet<String>) {
	if depth > 2 || out.len() >= MAX_RESULTS || !dir.is_dir() {
		return;
	}
	let entries = match std::fs::read_dir(dir) {
		Ok(e) => e,
		Err(_) => return,
	};
	for entry in entries.flatten() {
		if out.len() >= MAX_RESULTS {
			return;
		}
		let path = entry.path();
		if path.is_dir() {
			walk(&path, depth + 1, out, seen);
		} else if is_image(&path) {
			let key = path.to_string_lossy().to_ascii_lowercase();
			if seen.insert(key) {
				out.push(path);
			}
		}
	}
}

/// Decode an image file and return a downscaled JPEG `data:` URL, or None on any
/// read/decode/encode failure (so a single bad file never breaks the whole list).
fn thumbnail_data_url(path: &Path) -> Option<String> {
	let bytes = std::fs::read(path).ok()?;
	let img = image::load_from_memory(&bytes).ok()?;
	let (w, _) = img.dimensions();
	let scaled = if w > THUMB_WIDTH {
		img.resize(THUMB_WIDTH, THUMB_WIDTH, FilterType::Lanczos3)
	} else {
		img
	};
	let mut buf = std::io::Cursor::new(Vec::new());
	scaled.write_to(&mut buf, ImageFormat::Jpeg).ok()?;
	let b64 = base64::engine::general_purpose::STANDARD.encode(buf.into_inner());
	Some(format!("data:image/jpeg;base64,{}", b64))
}

#[tauri::command]
pub async fn get_wallpaper_thumbnails(app: AppHandle) -> Vec<WallpaperThumb> {
	let paths = collect_wallpapers(&app);
	let mut thumbs = Vec::with_capacity(paths.len());
	for path in paths {
		if let Some(data_url) = thumbnail_data_url(&path) {
			thumbs.push(WallpaperThumb {
				name: name_from_path(&path),
				path: path.to_string_lossy().to_string(),
				data_url,
			});
		}
	}
	thumbs
}

#[tauri::command]
pub fn set_wallpaper(path: String) -> Result<(), String> {
	if path.trim().is_empty() || !Path::new(&path).is_file() {
		return Err("Wallpaper file not found".into());
	}
	let wide: Vec<u16> = std::ffi::OsStr::new(&path)
		.encode_wide()
		.chain(std::iter::once(0))
		.collect();
	let result = unsafe {
		SystemParametersInfoW(
			SPI_SETDESKWALLPAPER,
			0,
			Some(wide.as_ptr() as *mut c_void),
			SPIF_UPDATEINIFILE | SPIF_SENDCHANGE,
		)
	};
	result.map_err(|e| format!("Failed to set wallpaper: {}", e))
}

#[tauri::command]
pub fn get_current_wallpaper() -> Option<String> {
	let mut buf = [0u16; 520];
	let result = unsafe {
		SystemParametersInfoW(
			SPI_GETDESKWALLPAPER,
			buf.len() as u32,
			Some(buf.as_mut_ptr() as *mut c_void),
			SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
		)
	};
	if result.is_err() {
		return None;
	}
	let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
	let path = String::from_utf16_lossy(&buf[..end]);
	if path.trim().is_empty() {
		None
	} else {
		Some(path)
	}
}
