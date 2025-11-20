use lopdf::{dictionary, Document, Object, Stream};
use lopdf::content::{Content, Operation};
use std::env;

fn main() -> lopdf::Result<()> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 3 {
        eprintln!("Usage: rpdf_merge <output.pdf> <input1.pdf> <input2.pdf> ...");
        std::process::exit(1);
    }

    let output_file = &args[1];
    let input_files = &args[2..];

    let mut merged = Document::with_version("1.5");
    let mut all_pages: Vec<(u32, u16)> = Vec::new();

    // Merge PDFs
    for file in input_files {
        let mut doc = Document::load(file)?;

        // Collect pages first
        all_pages.extend(doc.get_pages().values().copied());

        // Renumber and merge objects
        doc.renumber_objects_with(merged.max_id + 1);
        merged.max_id = doc.max_id;
        merged.objects.extend(doc.objects);
    }

    // Convert pages to Object::Reference for "Kids"
    let kids: Vec<Object> = all_pages.iter()
        .map(|&(num, r#gen)| Object::Reference((num, r#gen)))
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
        }.into()
    );

    merged.objects.insert(
        catalog_id,
        dictionary! {
            "Type" => "Catalog",
            "Pages" => pages_id
        }.into()
    );

    merged.trailer.set("Root", catalog_id);

    // Add page numbers
    for (i, page_id) in all_pages.iter().enumerate() {
        add_page_number(&mut merged, *page_id, i + 1, all_pages.len())?;
    }

    merged.compress();
    merged.save(output_file)?;

    println!("Merged PDF saved to {}", output_file);
    Ok(())
}

fn add_page_number(
    doc: &mut Document,
    page_id: (u32, u16),
    page_number: usize,
    _page_count: usize,
) -> lopdf::Result<()> {
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
        _ => vec![0.0, 0.0, 612.0, 792.0], // fallback default
    };

    let x0 = media_box[0];
    let y0 = media_box[1];
    let x1 = media_box[2];
    let _y1 = media_box[3];

    let width = x1 - x0;
    let bottom_center_x = x0 + width / 2.0;
    let text_y = y0 + 20.0;

    // Create page number content stream
    let content = lopdf::content::Content {
        operations: vec![
            lopdf::content::Operation::new("BT", vec![]),
            lopdf::content::Operation::new(
                "Tf",
                vec![Object::Name(b"Helvetica".to_vec()), Object::Real(12.0)],
            ),
            lopdf::content::Operation::new(
                "Td",
                vec![Object::Real(bottom_center_x as f32), Object::Real(text_y as f32)],
            ),
            lopdf::content::Operation::new(
                "Tj",
                vec![Object::string_literal(format!("{}", page_number))],
            ),
            lopdf::content::Operation::new("ET", vec![]),
        ],
    };

    let stream_id = doc.add_object(Stream::new(dictionary! {}, content.encode()?));

    // Attach stream to page Contents
    let page_obj_mut = doc.get_object_mut(page_id)?;
    let dict_mut = page_obj_mut.as_dict_mut()?;

    match dict_mut.get_mut(b"Contents") {
        Ok(Object::Reference(id)) => {
            *dict_mut.get_mut(b"Contents").unwrap() =
                Object::Array(vec![Object::Reference(*id), Object::Reference(stream_id)]);
        }
        Ok(Object::Array(arr)) => {
            arr.push(Object::Reference(stream_id));
        }
        Err(_) => {
            dict_mut.set("Contents", Object::Reference(stream_id));
        }
        Ok(_other) => {
            dict_mut.set("Contents", Object::Reference(stream_id));
        }
    }

    Ok(())
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
