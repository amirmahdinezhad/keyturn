//! Retina menu-bar metrics with account initials on the left.
use tauri::image::Image;
use crate::settings::MenuBarStyle;

#[derive(Clone)]
pub struct Metric {
    pub label: Option<String>,
    /// None is unavailable; zero is a real empty limit.
    pub fraction: Option<f64>,
    /// Reset-time position, oriented like the used/remaining value.
    pub time_marker: Option<f64>,
    pub color: [u8; 3],
}

#[derive(Clone)]
pub struct Label {
    pub text: String,
    pub caption: Option<String>,
    pub color: [u8; 3],
    pub segments: Vec<(String, [u8; 3])>,
    pub metrics: Vec<Metric>,
}

#[cfg(target_os = "macos")]
pub fn render(labels: &[Label], monochrome: bool, accent: [u8; 3], style: MenuBarStyle) -> Option<Image<'static>> {
    use objc2::{rc::autoreleasepool, runtime::AnyObject, AnyThread};
    use objc2_app_kit::{
        NSBitmapFormat, NSBitmapImageRep, NSColor, NSDeviceRGBColorSpace, NSFont,
        NSFontAttributeName, NSForegroundColorAttributeName, NSGraphicsContext, NSStringDrawing,
    };
    use objc2_foundation::{NSDictionary, NSPoint, NSString};
    autoreleasepool(|_| unsafe {
        let attrs = |size: f64, color: [u8; 3]| {
            let font = NSFont::systemFontOfSize(size);
            let color = NSColor::colorWithSRGBRed_green_blue_alpha(
                f64::from(color[0]) / 255.0, f64::from(color[1]) / 255.0,
                f64::from(color[2]) / 255.0, 1.0,
            );
            NSDictionary::from_slices(
                &[NSFontAttributeName, NSForegroundColorAttributeName],
                &[&*font as &AnyObject, &*color as &AnyObject],
            )
        };
        let layouts: Vec<_> = labels.iter().map(|label| {
            let progress = style == MenuBarStyle::Progress && !label.metrics.is_empty();
            let caption = label.caption.as_ref().filter(|s| !s.is_empty()).map(|s| NSString::from_str(s));
            let caption_attrs = attrs(25.0, if !label.segments.is_empty() { label.color } else if monochrome { accent } else { [170, 177, 185] });
            let caption_width = caption.as_ref().map(|s| s.sizeWithAttributes(Some(&caption_attrs)).width).unwrap_or(0.0);
            let caption_space = if caption.is_some() { caption_width.ceil() + 12.0 } else { 0.0 };
            let segments = if label.segments.is_empty() {
                vec![(label.text.clone(), if monochrome { accent } else { label.color })]
            } else { label.segments.clone() };
            let runs: Vec<_> = segments.iter().map(|(text, color)| {
                let text = NSString::from_str(text); let attributes = attrs(25.0, *color);
                let width = text.sizeWithAttributes(Some(&attributes)).width;
                (text, attributes, width)
            }).collect();
            let bar_label_attrs = attrs(if label.metrics.len() == 1 { 16.0 } else { 12.0 }, label.color);
            let bar_labels: Vec<_> = label.metrics.iter().map(|metric| {
                // Dense stacks retain exact bucket names in the tooltip.
                metric.label.as_ref().filter(|_| label.metrics.len() <= 3).map(|s| NSString::from_str(&s.chars().take(8).collect::<String>()))
            }).collect();
            let bar_label_width = bar_labels.iter().flatten().map(|s| s.sizeWithAttributes(Some(&bar_label_attrs)).width).fold(0.0, f64::max).ceil();
            let bar_label_space = if bar_label_width > 0.0 { bar_label_width + 8.0 } else { 0.0 };
            let content_width = if progress { bar_label_space + bar_width(label.metrics.len()) } else { runs.iter().map(|r| r.2).sum() };
            let width = (caption_space + content_width).ceil() as u32 + 12;
            (runs, caption, caption_attrs, caption_space, progress, bar_labels, bar_label_attrs, bar_label_space, width)
        }).collect();
        let width = 52 + layouts.iter().map(|layout| layout.8).sum::<u32>();
        let height = 36u32;
        let bitmap = NSBitmapImageRep::initWithBitmapDataPlanes_pixelsWide_pixelsHigh_bitsPerSample_samplesPerPixel_hasAlpha_isPlanar_colorSpaceName_bitmapFormat_bytesPerRow_bitsPerPixel(
            NSBitmapImageRep::alloc(), std::ptr::null_mut(), width as isize, height as isize, 8, 4, true, false, NSDeviceRGBColorSpace,
            NSBitmapFormat::empty(), (width*4) as isize, 32)?;
        let data = bitmap.bitmapData(); if data.is_null() { return None; }
        std::ptr::write_bytes(data, 0, (width * height * 4) as usize);
        let context = NSGraphicsContext::graphicsContextWithBitmapImageRep(&bitmap)?;
        NSGraphicsContext::saveGraphicsState_class(); NSGraphicsContext::setCurrentContext(Some(&context));
        let mut x = 58.0;
        for (label, layout) in labels.iter().zip(&layouts) {
            let (runs, caption, caption_attrs, caption_space, progress, bar_labels, bar_label_attrs, _, w) = layout;
            if let Some(caption) = caption {
                let size = caption.sizeWithAttributes(Some(caption_attrs));
                caption.drawAtPoint_withAttributes(NSPoint::new(x, (36.0 - size.height) / 2.0), Some(caption_attrs));
            }
            let mut metric_x = x + caption_space;
            if *progress {
                let (row_height, gap, top) = bar_stack(label.metrics.len());
                for (index, text) in bar_labels.iter().enumerate() {
                    if let Some(text) = text {
                        let size = text.sizeWithAttributes(Some(bar_label_attrs));
                        let center = top + index as f64 * (row_height + gap) + row_height / 2.0;
                        text.drawAtPoint_withAttributes(NSPoint::new(metric_x, 36.0 - center - size.height / 2.0), Some(bar_label_attrs));
                    }
                }
            } else {
                for (text, attributes, run_width) in runs {
                    text.drawAtPoint_withAttributes(NSPoint::new(metric_x, 2.0), Some(attributes));
                    metric_x += run_width;
                }
            }
            x += f64::from(*w);
        }
        NSGraphicsContext::restoreGraphicsState_class();
        let mut rgba = std::slice::from_raw_parts(data, (width * height * 4) as usize).to_vec();
        for pixel in rgba.chunks_exact_mut(4) {
            let alpha = u16::from(pixel[3]);
            if alpha > 0 { for channel in &mut pixel[..3] { *channel = (u16::from(*channel) * 255 / alpha).min(255) as u8; } }
        }
        let logo = crate::tray::render(accent);
        for y in 0..36u32 { for x in 0..46u32 {
            let source = ((y * 46 + x) * 4) as usize; let target = ((y * width + x) * 4) as usize;
            rgba[target..target + 3].copy_from_slice(&accent); rgba[target + 3] = logo.rgba()[source + 3];
        } }
        let mut x = 52u32;
        for (label, layout) in labels.iter().zip(&layouts) {
            if layout.4 {
                let bar_x = f64::from(x + 6) + layout.3 + layout.7;
                let (bar_height, gap, top) = bar_stack(label.metrics.len());
                let bar_width = bar_width(label.metrics.len());
                for (index, metric) in label.metrics.iter().enumerate() {
                    let y = top + index as f64 * (bar_height + gap);
                    rounded_bar(&mut rgba, width, bar_x, y, bar_width, bar_height, metric.color, 55);
                    if let Some(fraction) = metric.fraction.filter(|v| v.is_finite()) {
                        rounded_bar(&mut rgba, width, bar_x, y, bar_width * fraction.clamp(0.0, 1.0), bar_height, metric.color, 255);
                    } else {
                        rounded_bar(&mut rgba, width, bar_x + (bar_width - 8.0) / 2.0, y + (bar_height - 2.0) / 2.0, 8.0, 2.0, metric.color, 220);
                    }
                    if let Some(position) = metric.time_marker.filter(|v| v.is_finite() && (0.0..=1.0).contains(v)) {
                        let center = (bar_x + bar_width * position).clamp(bar_x + 2.0, bar_x + bar_width - 2.0);
                        // A neutral dark tick with a light halo stays visible on
                        // both the filled warning hue and the translucent track.
                        rounded_bar(&mut rgba, width, center - 2.0, y - 1.0, 4.0, bar_height + 2.0, [245, 247, 250], 245);
                        rounded_bar(&mut rgba, width, center - 1.0, y - 1.0, 2.0, bar_height + 2.0, [24, 26, 30], 255);
                    }
                }
            }
            x += layout.8;
        }
        Some(Image::new_owned(rgba, width, height))
    })
}

/// Two selected limits occupy two rows, within the existing 18pt menu-bar height.
#[cfg(target_os = "macos")]
fn bar_stack(count: usize) -> (f64, f64, f64) {
    if count == 1 { return (22.0, 0.0, 7.0); }
    let count = count.max(1) as f64;
    let gap = if count <= 4.0 { 4.0 } else { 1.0 };
    let height = ((26.0 - (count - 1.0) * gap) / count).clamp(0.5, 10.0);
    (height, gap, (36.0 - count * height - (count - 1.0) * gap) / 2.0)
}

#[cfg(target_os = "macos")]
fn bar_width(count: usize) -> f64 { if count == 1 { 96.0 } else { 72.0 } }

/// Rounded coverage is composed in straight alpha, matching Tauri's image buffer.
#[cfg(target_os = "macos")]
#[allow(clippy::too_many_arguments)]
fn rounded_bar(rgba: &mut [u8], image_width: u32, x: f64, y: f64, width: f64, height: f64, color: [u8; 3], alpha: u8) {
    if width <= 0.0 || height <= 0.0 { return; }
    let radius = (height / 2.0).min(width / 2.0);
    for py in y.floor().max(0.0) as u32..(y + height).ceil().min(36.0) as u32 {
        for px in x.floor().max(0.0) as u32..(x + width).ceil().min(f64::from(image_width)) as u32 {
            let mut coverage = 0.0;
            for sy in [0.25, 0.75] { for sx in [0.25, 0.75] {
                let fx = f64::from(px) + sx; let fy = f64::from(py) + sy;
                if fx < x || fx > x + width || fy < y || fy > y + height { continue; }
                let cx = fx.clamp(x + radius, x + width - radius);
                let cy = fy.clamp(y + radius, y + height - radius);
                if (fx - cx).powi(2) + (fy - cy).powi(2) <= radius.powi(2) { coverage += 0.25; }
            } }
            let offset = ((py * image_width + px) * 4) as usize;
            let foreground = f64::from(alpha) / 255.0 * coverage;
            let background = f64::from(rgba[offset + 3]) / 255.0;
            let out = foreground + background * (1.0 - foreground);
            if out > 0.0 {
                for channel in 0..3 {
                    rgba[offset + channel] = ((f64::from(color[channel]) * foreground + f64::from(rgba[offset + channel]) * background * (1.0 - foreground)) / out).round() as u8;
                }
                rgba[offset + 3] = (out * 255.0).round() as u8;
            }
        }
    }
}

#[cfg(not(target_os = "macos"))]
pub fn render(_labels: &[Label], _monochrome: bool, _accent: [u8; 3], _style: MenuBarStyle) -> Option<Image<'static>> { None }

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;

    #[test]
    fn reset_marker_is_drawn_on_every_progress_row_and_numeric_mode_is_unchanged() {
        for count in [1usize, 2, 3] {
            let metrics: Vec<_> = (0..count).map(|_| Metric { label: None, fraction: Some(0.25), time_marker: Some(0.5), color: [0, 122, 255] }).collect();
            let mut label = Label { text: "25%".into(), caption: None, color: [0, 122, 255], segments: Vec::new(), metrics };
            let image = render(&[label.clone()], true, [0, 122, 255], MenuBarStyle::Progress).unwrap();
            let marker_x = 58 + (bar_width(count) / 2.0) as u32;
            let (height, gap, top) = bar_stack(count);
            for row in 0..count {
                let y = (top + row as f64 * (height + gap) + height / 2.0) as u32;
                let offset = ((y * image.width() + marker_x) * 4) as usize;
                assert_eq!(&image.rgba()[offset..offset + 4], &[24, 26, 30, 255]);
            }
            let numeric = render(&[label.clone()], true, [0, 122, 255], MenuBarStyle::Percentages).unwrap();
            for metric in &mut label.metrics { metric.time_marker = None; }
            assert_eq!(numeric.rgba(), render(&[label], true, [0, 122, 255], MenuBarStyle::Percentages).unwrap().rgba());
        }
    }

    #[test]
    fn native_single_and_stacked_progress_time_marker_preview() {
        for count in [1usize, 2, 3] {
            let label = Label { text: "70%".into(), caption: Some("EB".into()), color: [0, 122, 255], segments: Vec::new(),
                metrics: (0..count).map(|index| Metric { label: Some(if index == 0 { "5h" } else { "1w" }.into()), fraction: Some(0.7 - index as f64 * 0.2), time_marker: Some(0.45 + index as f64 * 0.1), color: [0, 122, 255] }).collect() };
            let image = render(&[label], true, [0, 122, 255], MenuBarStyle::Progress).unwrap();
            if let Ok(path) = std::env::var("KEYTURN_TEST_TIME_MARKERS") {
                std::fs::write(format!("{path}-{count}.rgba"), image.rgba()).unwrap();
                std::fs::write(format!("{path}-{count}.json"), format!("[{},{}]", image.width(), image.height())).unwrap();
            }
        }
    }

    #[test]
    fn neither_percentages_nor_progress_draw_a_bottom_underline() {
        let label = Label { text: "5h 60%".into(), caption: Some("EB".into()), color: [0, 122, 255],
            segments: vec![("5h 60%".into(), [0, 122, 255])],
            metrics: vec![Metric { label: Some("5h".into()), fraction: Some(0.6), time_marker: None, color: [0, 122, 255] }] };
        for style in [MenuBarStyle::Percentages, MenuBarStyle::Progress] {
            let image = render(&[label.clone()], true, [0, 122, 255], style).unwrap();
            for y in 34..36 { for x in 52..image.width() {
                let pixel = ((y * image.width() + x) * 4) as usize;
                assert_eq!(image.rgba()[pixel + 3], 0, "no active underline in either display style");
            } }
        }
    }

    #[test]
    fn initials_match_percentage_font_size() {
        let label = Label { text: "88%".into(), caption: Some("88".into()), color: [0, 122, 255],
            segments: vec![("88%".into(), [255, 69, 58])], metrics: Vec::new() };
        let image = render(&[label], true, [0, 122, 255], MenuBarStyle::Percentages).unwrap();
        let height = |color: [u8; 3]| {
            let rows: Vec<_> = image.rgba().chunks_exact(4).enumerate().filter_map(|(index, pixel)| {
                let x = index as u32 % image.width(); let y = index as u32 / image.width();
                (x >= 52 && pixel[3] > 128 && pixel[..3] == color).then_some(y)
            }).collect();
            rows.iter().max().unwrap() - rows.iter().min().unwrap() + 1
        };
        assert_eq!(height([0, 122, 255]), height([255, 69, 58]), "initials and percentage digits need the same native type size");
    }

    #[test]
    fn progress_limits_stack_vertically_single_limit_is_larger_and_missing_stays_distinct() {
        let mut label = Label { text: "5h 50%  1w 25%".into(), caption: None, color: [0, 122, 255],
            segments: Vec::new(), metrics: vec![
                Metric { label: None, fraction: Some(0.5), time_marker: None, color: [0, 122, 255] },
                Metric { label: None, fraction: Some(0.25), time_marker: None, color: [255, 159, 10] },
            ] };
        let image = render(&[label.clone()], true, [0, 122, 255], MenuBarStyle::Progress).unwrap();
        let pixel = |image: &Image<'_>, x: u32, y: u32| { let i = ((y * image.width() + x) * 4) as usize; image.rgba()[i..i+4].to_vec() };
        assert_eq!(image.height(), 36);
        assert_eq!(pixel(&image, 88, 11), [0, 122, 255, 255]);
        assert_eq!(pixel(&image, 68, 25), [255, 159, 10, 255]);
        assert!(pixel(&image, 88, 25)[3] < 100, "shorter second bar must not borrow the first fraction");
        label.metrics.pop();
        assert!(render(&[label.clone()], true, [0, 122, 255], MenuBarStyle::Progress).unwrap().width() > image.width());
        assert!(bar_stack(1).0 > bar_stack(2).0);
        label.metrics[0].fraction = Some(0.0);
        let empty = render(&[label.clone()], true, [0, 122, 255], MenuBarStyle::Progress).unwrap();
        label.metrics[0].fraction = None;
        let missing = render(&[label], true, [0, 122, 255], MenuBarStyle::Progress).unwrap();
        assert!(pixel(&empty, 106, 18)[3] < 100);
        assert!(pixel(&missing, 106, 18)[3] > 200, "unknown usage has an unavailable dash rather than a zero reading");
    }

    #[test]
    fn initials_are_left_of_metrics_in_both_display_styles() {
        let label = Label { text: "25%".into(), caption: Some("EB".into()), color: [0, 122, 255],
            segments: vec![("25%".into(), [255, 69, 58])],
            metrics: vec![Metric { label: None, fraction: Some(0.25), time_marker: None, color: [255, 69, 58] }] };
        for style in [MenuBarStyle::Percentages, MenuBarStyle::Progress] {
            let image = render(&[label.clone()], true, [0, 122, 255], style).unwrap();
            let colored_x = |color: [u8; 3]| image.rgba().chunks_exact(4).enumerate().filter_map(|(i,p)| {
                let x = i as u32 % image.width();
                (x >= 52 && p[3] > 240 && p[..3] == color).then_some(x)
            }).collect::<Vec<_>>();
            let initials = colored_x([0, 122, 255]); let metrics = colored_x([255, 69, 58]);
            assert!(!initials.is_empty() && !metrics.is_empty());
            assert!(initials.iter().max().unwrap() < metrics.iter().min().unwrap(), "initials belong on the left, not below the metrics");
        }
    }

    #[test]
    fn native_progress_preview_with_two_selected_limits() {
        let label = Label { text: "5h 70%  1w 4%".into(), caption: Some("EB".into()), color: [0, 122, 255],
            segments: vec![("5h 70%".into(), [0, 122, 255]), ("  1w 4%".into(), [255, 69, 58])],
            metrics: vec![Metric { label: Some("5h".into()), fraction: Some(0.7), time_marker: None, color: [0, 122, 255] }, Metric { label: Some("1w".into()), fraction: Some(0.04), time_marker: None, color: [255, 69, 58] }] };
        for (suffix, style) in [("progress", MenuBarStyle::Progress), ("percentages", MenuBarStyle::Percentages)] {
            let image = render(&[label.clone()], true, [255, 69, 58], style).unwrap();
            if style == MenuBarStyle::Progress {
                let last_bar_pixel = ((34 * image.width() + image.width() - 10) * 4) as usize;
                assert_eq!(image.rgba()[last_bar_pixel + 3], 0, "active indicator must not create a third bar");
            }
            if let Ok(path) = std::env::var("KEYTURN_TEST_TRAY_STYLES") {
                std::fs::write(format!("{path}-{suffix}.rgba"), image.rgba()).unwrap();
                std::fs::write(format!("{path}-{suffix}.json"), format!("[{},{}]", image.width(), image.height())).unwrap();
            }
        }
    }

    #[test]
    fn accent_colors_monochrome_metrics_and_preserves_status_colors() {
        let accent = [124, 108, 240];
        let labels = [Label {
            text: "5h 60%".into(),
            caption: Some("Work".into()),
            color: [48, 184, 107],
           
            segments: Vec::new(), metrics: Vec::new(),
        }];
        for monochrome in [true, false] {
            let image = render(&labels, monochrome, accent, MenuBarStyle::Percentages).unwrap();
            let region_has = |color: [u8; 3], text: bool| {
                image.rgba().chunks_exact(4).enumerate().any(|(i, p)| {
                    let x = i as u32 % image.width();
                    let y = i as u32 / image.width();
                    p[3] > 240 && p[..3] == color && (if text { x >= 52 && y < 34 } else { x < 46 })
                })
            };
            assert!(region_has(accent, false), "logo must use the accent");
            assert!(region_has(if monochrome { accent } else { labels[0].color }, true));
            let bottom = ((34 * image.width() + 58) * 4) as usize;
            assert_eq!(image.rgba()[bottom + 3], 0, "menu-bar text has no underline");
        }
    }

    #[test]
    fn system_font_percentages_render_to_a_transparent_retina_image() {
        let labels = vec![Label {
            text: "5h 60%".into(),
            caption: Some("Work".into()),
            color: [48, 184, 107],
           
            segments: Vec::new(), metrics: Vec::new(),
        }];
        let image = render(&labels, false, crate::tray::DEFAULT_ACCENT, MenuBarStyle::Percentages).expect("native text renderer");
        if let Ok(path) = std::env::var("KEYTURN_TEST_TRAY_RGBA") {
            std::fs::write(&path, image.rgba()).unwrap();
            std::fs::write(format!("{path}.json"), format!("[{},{}]", image.width(), image.height())).unwrap();
        }
        assert_eq!(image.height(), 36);
        assert!(image.width() > 100);
        let pixels: Vec<_> = image.rgba().chunks_exact(4).collect();
        assert!(pixels.iter().any(|p| p[3] == 0));
        assert!(pixels.iter().any(|p| p[1] > p[0] && p[3] > 0));
        assert!(pixels.iter().any(|p| p[3] > 128));
    }
}
