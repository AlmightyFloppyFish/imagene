mod action;
mod cli;
use action::Action::*;
use action::{Direction, Flag, Orientation};
use image::*;
use std::collections::HashMap;
use std::sync::mpsc;

fn main() {
    let (io, settings, image_names) = cli::parse();

    // TODO: If you append the same image as source it'll error, i should check if already exists
    // or something
    let mut images: HashMap<String, mpsc::Receiver<DynamicImage>> = HashMap::new();
    let mut extra_images: HashMap<String, DynamicImage> = HashMap::new();
    for image_name in image_names {
        if images.contains_key(&image_name) {
            continue;
        }
        let (s, r) = mpsc::channel();
        let i_n = image_name.clone();
        std::thread::spawn(move || {
            s.send(
                image::open(&i_n)
                    .map_err(|e| {
                        println!("{}", e);
                    })
                    .expect("Aborting because one or more errors while loading image"),
            )
            .unwrap();
        });
        images.insert(image_name, r);
    }

    let mut image = images.get_mut(&io.0).unwrap().recv().unwrap();
    for action in settings.actions {
        match action {
            Contrast(c) => image = image.adjust_contrast(c),
            Brightness(b) => image = image.brighten(b),
            Blur(b) => image = image.blur(b),
            Unsharpen(sigma, threshold) => image = image.unsharpen(sigma, threshold),
            Crop(x, y, w, h) => image = image.crop(x, y, w, h),
            Rotate(d) => {
                image = match d {
                    Direction::Right => image.rotate90(),
                    Direction::Left => image.rotate270(),
                    Direction::Down => image.rotate180(),
                }
            }
            Flip(orientation) => match orientation {
                Orientation::Vertical => image = image.flipv(),
                Orientation::Horizontal => image = image.fliph(),
            },
            Scale(w, h) => {
                if w == 0 {
                    image = image.resize(100000000, h, Nearest);
                    continue;
                }
                if h == 0 {
                    image = image.resize(w, 100000000, Nearest);
                    continue;
                }
                image = image.resize_exact(w, h, Nearest)
            }
            Append(filename) => {
                // The appendable image can either be source, an image that hasn't been
                // initialized, or an already initialized one. This handles all 3 cases
                let image_to_append;
                if filename == io.0 {
                    image_to_append = image.clone();
                } else {
                    if !extra_images.contains_key(&filename) {
                        extra_images.insert(
                            filename.clone(),
                            images
                                .get_mut(&filename)
                                .unwrap()
                                .recv()
                                .map_err(|e| println!("{}", e))
                                .unwrap(),
                        );
                    }
                    image_to_append = extra_images.get_mut(&filename).unwrap().clone();
                }

                let (original_resized, resized_image) = if match settings.flags.get(&Flag::Shrink) {
                    Some(f) => *f,
                    None => false,
                } {
                    if image_to_append.height() > image.height() {
                        (
                            true,
                            image.resize(100000000, image_to_append.height(), Nearest),
                        )
                    } else {
                        (
                            false,
                            image_to_append.resize(100000000, image.height(), Nearest),
                        )
                    }
                } else {
                    if image_to_append.height() > image.height() {
                        (
                            false,
                            image_to_append.resize(100000000, image.height(), Nearest),
                        )
                    } else {
                        (
                            true,
                            image.resize(100000000, image_to_append.height(), Nearest),
                        )
                    }
                };
                let mut parent = image::DynamicImage::new_rgba8(
                    image.width() + image_to_append.width(),
                    image.height(),
                );
                if original_resized {
                    parent.copy_from(&resized_image, 0, 0);
                    parent.copy_from(&image_to_append, image.width(), 0);
                } else {
                    parent.copy_from(&image, 0, 0);
                    parent.copy_from(&resized_image, image.width(), 0);
                }
                image = parent;
            }
        };
    }

    match io.1.as_ref() {
        "stdout" => image
            .write_to(&mut std::io::stdout(), ImageOutputFormat::PNG)
            .unwrap(),
        _ => image.save(&io.1).unwrap(),
    }
}
