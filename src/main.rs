use eframe::egui;
use lopdf::{dictionary, Document, Object, Stream};
use lopdf::content::{Content, Operation};
use rfd::FileDialog;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions::default();
    eframe::run_native(
        "PDF Merger",
        options,
        Box::new(|_cc| Ok(Box::new(MergeApp::default()))),
    )
}


#[derive(Clone, Copy, PartialEq, Debug)]
pub enum MergeMode {
    MergeOnly,
    MergeAndNumber,
}

struct MergeApp {
    input_files: Vec<String>,
    output_file: Option<String>,
    merge_status: String,
    mode: MergeMode,
}

impl Default for MergeApp {
    fn default() -> Self {
        Self {
            input_files: Vec::new(),
            output_file: None,
            merge_status: String::new(),
            mode: MergeMode::MergeAndNumber,
        }
    }
}

impl eframe::App for MergeApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("PDF Merger");

            ui.separator();

            ui.horizontal(|ui| {
                ui.label("Mode:");

                ui.radio_value(
                    &mut self.mode,
                    MergeMode::MergeAndNumber,
                    "Merge + Number pages",
                );

                ui.radio_value(
                    &mut self.mode,
                    MergeMode::MergeOnly,
                    "Merge only",
                );
            });

            ui.separator();

            // Select input files to merge and number
            ui.label("Input PDF files:");
            for file in &self.input_files {
                ui.monospace(format!("• {}", file));
            }

            if ui.button("Add input files...").clicked() {
                if let Some(files) = FileDialog::new()
                    .add_filter("PDF", &["pdf"])
                    .pick_files()
                {
                    // IMPORTANT: append rather than replace
                    for file in files {
                        self.input_files.push(file.display().to_string());
                    }
                }
            }

            ui.separator();

            // Select the output file
            ui.label("Output PDF file:");
            if let Some(out) = &self.output_file {
                ui.monospace(out);
            }

            if ui.button("Choose output file...").clicked() {
                if let Some(file) = FileDialog::new()
                    .add_filter("PDF", &["pdf"])
                    .save_file()
                {
                    self.output_file = Some(file.display().to_string());
                }
            }

            ui.separator();

            // Merge the files together
            if ui.button("Merge PDFs").clicked() {

                if self.input_files.is_empty() {
                    self.merge_status = "Error: No input files selected".into();
                } else if self.output_file.is_none() {
                    self.merge_status = "Error: No output file selected".into();
                } else {
                    match merge_and_number_files(
                        self.input_files.clone(),
                        self.output_file.clone().unwrap(),
                        self.mode,
                    ) {
                        Ok(_) => self.merge_status = "Merge complete!".into(),
                        Err(e) => self.merge_status = format!("Error: {}", e),
                    }
                }
            }

            ui.separator();

            ui.label(&self.merge_status);
        });
    }
}

/// Merge the files together and number them A1 ... A3, B1 ... B5, etc.
pub fn merge_and_number_files(input_files: Vec<String>, output_file: String, mode: MergeMode) -> Result<(), Box<dyn std::error::Error>> {
    let (all_pages, mut merged, page_prefix_number) =
        process_input_files(&input_files)?;

    build_pages_and_catalog(&mut merged, &all_pages)?;

    // Add page numbers
    if mode == MergeMode::MergeAndNumber {
        for (i, page_id) in all_pages.iter().enumerate() {
            add_page_number(&mut merged, *page_id, i + 1, &page_prefix_number)?;
        }
    }

    cleanup(&mut merged, &output_file)?;

    Ok(())
}

/// Finish up with the file. Compress it then save to the output file
/// that was given.
fn cleanup(merged: &mut Document, output_file: &str) -> Result<(), Box<dyn std::error::Error>> {
    merged.compress();
    merged.save(output_file)?;

    Ok(())
}


/// Loads all input PDFs, renumbers objects, collects pages,
/// generates prefix page numbers, and merges into `merged`.
fn process_input_files(
    input_files: &[String],
) -> Result<(Vec<(u32, u16)>, Document, Vec<String>), Box<dyn std::error::Error>> {
    let mut merged = Document::with_version("1.5");
    let mut all_pages: Vec<(u32, u16)> = Vec::new();
    let mut page_prefix_number: Vec<String> = Vec::new();

    for (file_index, file) in input_files.iter().enumerate() {
        let mut doc = Document::load(file)?;

        // Compute prefix letter ('A', 'B', ...)
        let prefix = ((file_index as u8) + b'A') as char;

        // Renumber objects to avoid collisions
        doc.renumber_objects_with(merged.max_id + 1);
        merged.max_id = doc.max_id;

        // Collect page IDs AFTER renumbering
        let page_ids: Vec<(u32, u16)> = doc.get_pages().values().copied().collect();

        // Generate prefix + page number strings (A1, A2, B1, ...)
        for (i, _) in page_ids.iter().enumerate() {
            page_prefix_number.push(format!("{}{}", prefix, i + 1));
        }

        // Append to list of all pages
        all_pages.extend(page_ids.iter().copied());

        // Merge objects into base PDF
        merged.objects.extend(doc.objects);
    }

    Ok((all_pages, merged, page_prefix_number))
}

fn build_pages_and_catalog(
    merged: &mut Document,
    all_pages: &Vec<(u32, u16)>
) -> lopdf::Result<()> {
    // Convert pages to Object::Reference for "Kids"
    let kids: Vec<Object> = all_pages
        .iter()
        .map(|&page_id| Object::Reference(page_id))
        .collect();

    // Create Pages and Catalog objects
    let pages_id = merged.new_object_id();
    let catalog_id = merged.new_object_id();

    merged.objects.insert(
        pages_id,
        dictionary! {
            "Type" => "Pages",
            "Kids" => kids,
            "Count" => all_pages.len() as i32
        }
        .into(),
    );

    merged.objects.insert(
        catalog_id,
        dictionary! {
            "Type" => "Catalog",
            "Pages" => pages_id
        }
        .into(),
    );

    merged.trailer.set("Root", catalog_id);

    Ok(())
}

/// Add the page number and prefix to the page.
fn add_page_number(
    doc: &mut Document,
    page_id: (u32, u16),
    page_number: usize,
    page_prefix_number: &[String],
) -> lopdf::Result<()> {
    let media_box = get_media_box(doc, page_id)?;

    let content = create_page_number_content(
        &media_box,
        page_prefix_number,
        page_number,
    );

    let stream_id = doc.add_object(Stream::new(dictionary! {}, content.encode()?));

    attach_stream_to_page(doc, page_id, stream_id)?;

    Ok(())
}


fn attach_stream_to_page(
    doc: &mut Document,
    page_id: (u32, u16),
    stream_id: (u32, u16),
) -> lopdf::Result<()> {
    // Get mutable page object
    let page_obj_mut = doc.get_object_mut(page_id)?;
    let dict_mut = page_obj_mut.as_dict_mut()?;

    match dict_mut.get_mut(b"Contents") {
        // Case: "Contents" is currently a *single reference*
        Ok(Object::Reference(existing_id)) => {
            // Replace it with an array containing the old + new stream
            *dict_mut.get_mut(b"Contents").unwrap() =
                Object::Array(vec![
                    Object::Reference(*existing_id),
                    Object::Reference(stream_id),
                ]);
        }

        // Case: "Contents" is already an array
        Ok(Object::Array(arr)) => {
            arr.push(Object::Reference(stream_id));
        }

        // Case: "Contents" does not exist
        Err(_) => {
            dict_mut.set("Contents", Object::Reference(stream_id));
        }

        // Fallback: Anything else weird → override with the new stream
        Ok(_) => {
            dict_mut.set("Contents", Object::Reference(stream_id));
        }
    }

    Ok(())
}

/// Get the media box
fn get_media_box(
    doc: &Document,
    page_id: (u32, u16),
) -> lopdf::Result<Vec<f64>> {
    let page_obj = doc.get_object(page_id)?;
    let dict = page_obj.as_dict()?;

    // default MediaBox (8.5 x 11 in)
    let default_box = Object::Array(vec![
        Object::Integer(0),
        Object::Integer(0),
        Object::Integer(612),
        Object::Integer(792),
    ]);

    // Get MediaBox or CropBox; use default if missing
    let media_box_obj = dict
        .get(b"MediaBox")
        .or_else(|_| dict.get(b"CropBox"))
        .unwrap_or(&default_box);

    let media_box = match media_box_obj {
        Object::Array(arr) => nums_from_array(arr)?,
        _ => vec![0.0, 0.0, 612.0, 792.0],
    };

    Ok(media_box)
}

/// Create the content
fn create_page_number_content(
    media_box: &[f64],
    page_prefix_number: &[String],
    page_number: usize,
) -> Content {
    let x0 = media_box[0];
    let y0 = media_box[1];
    let x1 = media_box[2];
    // let y1 = media_box[3]; // unused

    let width = x1 - x0;
    let bottom_center_x = x0 + width / 2.0;
    let text_y = y0 + 20.0;

    Content {
        operations: vec![
            Operation::new("BT", vec![]),
            Operation::new(
                "Tf",
                vec![Object::Name(b"Helvetica".to_vec()), Object::Real(12.0)],
            ),
            Operation::new(
                "Td",
                vec![
                    Object::Real(bottom_center_x as f32),
                    Object::Real(text_y as f32),
                ],
            ),
            Operation::new(
                "Tj",
                vec![Object::string_literal(
                    page_prefix_number[page_number - 1].clone(),
                )],
            ),
            Operation::new("ET", vec![]),
        ],
    }
}

// Helper to convert PDF numeric array to Vec<f64>
fn nums_from_array(arr: &Vec<Object>) -> lopdf::Result<Vec<f64>> {
    arr.iter()
        .map(|obj| match obj {
            Object::Integer(i) => Ok(*i as f64),
            Object::Real(r) => Ok(*r as f64),
            _ => Ok(0.0),
        })
        .collect()
}
