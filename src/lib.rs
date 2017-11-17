#[macro_use]
extern crate serde_derive;
extern crate image;
extern crate tempdir;


use tempdir::*;

pub mod data;
mod fonts;
pub mod options;
pub mod utils;

pub use data::*;
pub use options::*;
pub use utils::*;

use std::collections::hash_map::DefaultHasher;
use std::fs::File;
use std::hash::{Hash, Hasher};
use std::path::*;

use image::{ImageBuffer, Pixel};

/// OHLC Chart Configuration, mutate through the methods
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct OHLCRenderOptions {
	/// Title of the chart
	/// Currently ignored
	pub(crate) title: String,
	/// Currently ignored
	/// Colour for the title of the chart
	pub(crate) title_colour: u32,
	/// Background colour of the entire chart
	pub(crate) background_colour: u32,
	/// Colour for the "current value" dot and line across the chart
	pub(crate) current_value_colour: u32,
	/// The prefix for the values represented in the OHLC
	/// Currently ignored
	pub(crate) value_prefix: String,
	/// The suffix for the values represented in the OHLC
	/// Currently ignored
	pub(crate) value_suffix: String,
	/// The amount of time, in seconds, each OHLC objects represent
	/// Currently ignored
	pub(crate) time_units: u64,
	/// Options for the horizontal axis
	/// Currently ignored
	pub(crate) h_axis_options: AxisOptions,
	/// Options for the vertical axis
	pub(crate) v_axis_options: AxisOptions,
	/// RGBA(8) Colour for when the OHLC indicates fall
	pub(crate) down_colour: u32,
	/// RGBA(8) Colour for when the OHLC indicates rise
	pub(crate) up_colour: u32,
}

impl OHLCRenderOptions {
	/// Creates an object for render options with default parameters
	pub fn new() -> OHLCRenderOptions {
		OHLCRenderOptions {
			title: String::new(),
			title_colour: 0,
			background_colour: 0xDDDDDDFF,
			// Bright-ass blue
			current_value_colour: 0x0000FFFF,
			value_prefix: String::new(),
			value_suffix: String::new(),
			// Default is 1 hour
			time_units: 3600,
			h_axis_options: AxisOptions::new(),
			v_axis_options: AxisOptions::new(),
			// Bright-ass red
			down_colour: 0xFF0000FF,
			// Bright-ass green
			up_colour: 0x00FF00FF,
		}
	}

	pub fn indicator_colours(mut self, current_val: u32, down: u32, up: u32) -> Self {
		self.current_value_colour = current_val;
		self.down_colour = down;
		self.up_colour = up;

		self
	}

	pub fn background_colour(mut self, colour: u32) -> Self {
		self.background_colour = colour;

		self
	}

	pub fn h_axis<F>(mut self, mut f: F) -> Self
		where F: FnMut(AxisOptions) -> AxisOptions {
		self.h_axis_options = (f)(self.h_axis_options);

		self
	}

	pub fn v_axis<F>(mut self, mut f: F) -> Self
		where F: FnMut(AxisOptions) -> AxisOptions {
		self.v_axis_options = (f)(self.v_axis_options);

		self
	}

	/// Renders the OHLC Chart by the data, using the configs provided.
	///
	/// Takes a lambda function for processing the image once it's rendered, do not do anything asynchronous with the image as it will be deleted as soon as the function finishes.
	///
	/// Returns an error string originating from OHLC if an error occurs, and the result of the callback function otherwise.
	pub fn render<F>(&self, data: Vec<OHLC>, callback: F) -> Result<Result<(), String>, String>
		where F: Fn(&Path) -> Result<(), String> + Sized {
		let mut hasher = DefaultHasher::new();
		data.hash(&mut hasher);

		// Create temporary directory
		if let Ok(dir) = TempDir::new(&format!("ohlc_render_{}", hasher.finish())) {
			let file_path = dir.path().join("chart.png");

			let mut result = match self.render_and_save(data, &file_path) {
				Ok(_) => Ok((callback)(&file_path)),
				Err(err) => Err(err)
			};

			let _ = dir.close(); // Delete temporary directory

			result
		} else {
			Err("Failed to create a temporary directory.".to_string())
		}
	}

	/// Renders the chart and saves it to the specified path
	///
	/// Returns an error string if an error occurs
	pub fn render_and_save(&self, data: Vec<OHLC>, path: &Path) -> Result<(), String> {
		if let Err(err) = validate(&data) {
			return Err(format!("Data validation error: {}", err));
		}

		// String.bytes, top edge x, leftmost edge y, colour, do a border
		let mut text_renders: Vec<(Vec<u8>, u32, u32, u32, bool)> = vec![];

		let ohlc_of_set = calculate_ohlc_of_set(&data);

		let margin_top = 60u32;
		let margin_bottom = 35u32;
		let margin_left = 0u32;
		let margin_right = 90u32;

		let width = 1280;
		let height = 720;

		let mut image_buffer: ImageBuffer<image::Rgba<u8>, _> = ImageBuffer::new(width, height);

		// Filling the background here
		if self.background_colour % 256 > 0 {
			for x in 0..width {
				for y in 0..height {
					let mut chs = image_buffer
						.get_pixel_mut(x, y)
						.channels_mut();
					for j in 0..4 {
						chs[3 - j] = (self.background_colour >> (8 * j)) as u8;
					}
				}
			}
		}

		let candle_width = ((width - (margin_left + margin_right)) as f64 / data.len() as f64).floor();
		let stick_width = (|x| if x < 1 && candle_width >= 3. { 1 } else { x })((candle_width / 10. + 0.3).round() as u32);

		let y_val_increment = ohlc_of_set.range() / (height - (margin_top + margin_bottom)) as f64;

		// Rendering the lines occur here
		if self.v_axis_options.line_colour % 256 > 0 && self.v_axis_options.line_frequency > 0. {
			for y_es in 0..(height - (margin_top + margin_bottom)) {
				if (|d| d < y_val_increment && d >= 0.)((ohlc_of_set.h - y_es as f64 * y_val_increment) % self.v_axis_options.line_frequency) {
					let y = y_es + margin_top;
					for x in 0..(width - (margin_left + margin_right)) {
						let mut chs = image_buffer
							.get_pixel_mut(x, y)
							.channels_mut();
						for j in 0..4 {
							chs[3 - j] = (self.v_axis_options.line_colour >> (8 * j)) as u8;
						}
					}
				}

				// Rendering text for the lines occur here
				if self.v_axis_options.label_colour % 256 != 0 && (|d| d < y_val_increment && d >= 0.)((ohlc_of_set.h - y_es as f64 * y_val_increment) % self.v_axis_options.label_frequency) {
					let base_y = y_es + margin_top - 8; // Top edge...

					let mut chars = format!("{}", ((ohlc_of_set.h - y_es as f64 * y_val_increment) / self.v_axis_options.label_frequency).round() * self.v_axis_options.label_frequency).into_bytes();

					while chars.len() > ((margin_right as f32 - 10.) / 10.).floor() as usize {
						let _ = chars.pop();
					}
					text_renders.push((chars, width - margin_right + 10u32, base_y, self.v_axis_options.label_colour, true))
				}
			}
		}

		// The below section renders the OHLC candles
		for (i, ohlc_elem) in data.iter().enumerate() {
			let colour = if ohlc_elem.o > ohlc_elem.c { self.down_colour } else { self.up_colour };

			// Yes, no left margin
			let begin_pos = (candle_width * i as f64) as u32;
			let end_pos = (candle_width * (i + 1) as f64) as u32;

			let open_ys = ((ohlc_elem.o - ohlc_of_set.l) / y_val_increment).round() as u32;
			let close_ys = ((ohlc_elem.c - ohlc_of_set.l) / y_val_increment).round() as u32;

			let x_center = (((begin_pos + end_pos) as f64) / 2.).round() as u32;

			// Candles are rendered inside here
			for y_state in if open_ys > close_ys { close_ys..(1 + open_ys) } else { open_ys..(1 + close_ys) } {
				let y = height - y_state - margin_bottom;
				// Introduce right padding if the candle isn't too short
				for x in begin_pos..(if end_pos - begin_pos > 3 { end_pos - 1 } else { end_pos + 1 }) {
					let mut chs = image_buffer
						.get_pixel_mut(x, y)
						.channels_mut();
					for j in 0..4 {
						chs[3 - j] = (colour >> (8 * j)) as u8;
					}
				}
			}

			// Sticks and rendered inside here
			for y_state in (((ohlc_elem.l - ohlc_of_set.l) / y_val_increment).round() as u32)..(1 + ((ohlc_elem.h - ohlc_of_set.l) / y_val_increment).round() as u32) {
				let y = height - y_state - margin_bottom;

				for x in (x_center - stick_width - 1) as u32..(x_center + stick_width - 1) as u32 {
					let mut chs = image_buffer
						.get_pixel_mut(x, y)
						.channels_mut();
					for j in 0..4 {
						chs[3 - j] = (colour >> (8 * j)) as u8;
					}
				}
			}

			// Current value line is rendered inside here.
			if i == data.len() - 1 {
				let y = height - (((ohlc_of_set.c - ohlc_of_set.l) / y_val_increment).round() as u32) - margin_bottom;
				for x in margin_left..(width - margin_right) {
					let mut chs = image_buffer
						.get_pixel_mut(x, y)
						.channels_mut();
					for j in 0..4 {
						chs[3 - j] = (self.current_value_colour >> (8 * j)) as u8;
					}
				}

				for x_offset in -2i32..3 {
					for y_offset in -2i32..3 {
						if !(x_offset == y_offset || x_offset + y_offset == 0 || x_offset == 0) { continue }

						let mut chs = image_buffer
							.get_pixel_mut((x_offset + (x_center as i32)) as u32, (y_offset + (y as i32)) as u32)
							.channels_mut();
						for j in 0..4 {
							chs[3 - j] = (self.current_value_colour >> (8 * j)) as u8;
						}
					}
				}

				// Add label to the closing value
				{
					let mut chars = format!("{}", ohlc_of_set.c).into_bytes();

					while chars.len() > ((margin_right as f32 - 10.) / 10.).floor() as usize {
						let _ = chars.pop();
					}
					text_renders.push((chars, width - margin_right + 10u32, y - 8, self.current_value_colour, true))
				}
			}
		}

		text_renders.push((self.title.clone().into_bytes(), 0, 0, self.title_colour, false));

		// Text renderer section
		for (chars, base_x, base_y, colour, do_border) in text_renders {
			// 10 is character width; f_x is starting at the left edge of the margin
			for f_x in 0usize..chars.len() {
				let char_font: &[u8; 170] = &fonts::ASCII_TABLE[chars[(|d| if d < 127 { d } else { 0x20 })(f_x)] as usize];
				for incr_y in 0usize..17 {
					for incr_x in 0usize..10 {
						let x = base_x + (incr_x + f_x * 10) as u32;
						let y = base_y + incr_y as u32;

						if do_border {
							// TODO draw a box with the predetermined label colour
							let mut paint_cords: Option<(u32, u32)> = None;
							if incr_y == 0 {
								paint_cords = Some((x, y - 1));
							} else if incr_y == 16 {
								paint_cords = Some((x, y + 1));
							}
							if incr_x == 0 {
								paint_cords = Some((x - 1, paint_cords.unwrap_or((0, y)).1));
							} else if incr_x == 9 {
								paint_cords = Some((x + 1, paint_cords.unwrap_or((0, y)).1));
							}

							if let Some((x, y)) = paint_cords {
								let mut chs = image_buffer
									.get_pixel_mut(x, y)
									.channels_mut();
								for j in 0..4 {
									chs[3 - j] = (colour >> (8 * j)) as u8;
								}
							}
						}

						let shade_at_pos = char_font[incr_x + incr_y * 10] as u32;

						if shade_at_pos == 0 {
							let mut chs = image_buffer
								.get_pixel_mut(x, y)
								.channels_mut();
							for j in 0..4 {
								chs[3 - j] = (self.background_colour >> (8 * j)) as u8;
							}
							continue;
						}

						let mut chs = image_buffer
							// Translate right 10px and up 17px otherwise it'd look weird ass
							.get_pixel_mut(x, y)
							.channels_mut();

						// Don't modify the alpha channel
						for j in 1..4 {
							let bge = (self.background_colour >> (8 * j)) as u8;
							let curr_col = (colour >> (8 * j)) as u8;

							chs[3 - j] = (
								((shade_at_pos * curr_col as u32 +
									// Add the existing background instead of doing alphas
									((0xff - shade_at_pos) * bge as u32)
								) as f64
									/ 255.
								).round()) as u8;
						}
					}
				}
			}
		}

		// File save occurs here
		match File::create(path) {
			Ok(ref mut file) => match image::ImageRgba8(image_buffer).save(file, image::PNG) {
				Ok(_) => Ok(()),
				Err(err) => Err(format!("Image write error: {:?}", err))
			}
			Err(err) => Err(format!("File create error: {:?}", err))
		}
	}
}

fn validate(data: &Vec<OHLC>) -> Result<(), &'static str> {
	for elem in data {
		return if elem.o > elem.h {
			Err("Opening value is higher than high value.")
		} else if elem.c > elem.h {
			Err("Closing value is higher than high value.")
		} else if elem.l > elem.h {
			Err("Low value is higher than high value.")
		} else if elem.o < elem.l {
			Err("Opening value is lower than low value.")
		} else if elem.c < elem.l {
			Err("Closing value is lower than low value.")
		} else {
			continue
		};
	}
	Ok(())
}

#[cfg(test)]
mod tests {
	extern crate serde_json;

	// use std::fs;
	use std::io::{Read, Write};
	use super::*;
	use image::GenericImage;

	#[test]
	fn render_options_modification() {
		assert_eq!(
			OHLCRenderOptions {
				title: String::new(),
				title_colour: 0,
				background_colour: 0xFEFEFEFE,
				current_value_colour: 0x69696968,
				value_prefix: String::new(),
				value_suffix: String::new(),
				time_units: 3600,
				h_axis_options: AxisOptions::new(),
				v_axis_options: AxisOptions::new(),
				down_colour: 0x69696969,
				up_colour: 0x69696970,
			},
			OHLCRenderOptions::new()
				.indicator_colours(0x69696968, 0x69696969, 0x69696970)
				.background_colour(0xFEFEFEFE)
		);
	}

	#[test]
	fn axis_options_modification() {
		assert_eq!(
			AxisOptions {
				title: "I'm a meme".to_string(),
				title_colour: 69,
				line_colour: 70,
				line_frequency: 71.,
				label_colour: 72,
				label_frequency: 73.,
			},
			AxisOptions::new()
				.title("I'm a meme")
				.title_colour(69)
				.line_colour(70)
				.line_frequency(71.)
				.label_colour(72)
				.label_frequency(73.)
		);
	}

	#[test]
	fn render_draw_sample_data() {
		let mut buf = String::new();
		let _ = File::open("sample_data.json").unwrap().read_to_string(&mut buf);
		let _ = OHLCRenderOptions::new()
			.v_axis(|va| va
				.line_colour(0x000000FF)
				.line_frequency(200.)
				.label_colour(0x0000FF)
				.label_frequency(200.)
			)
			.render_and_save(
				self::serde_json::from_str(&buf).unwrap(),
				&Path::new("test-draw-sample-data.png")
			);
	}

	/*
		#[test]
		fn render_repetition() {
			let _ = OHLCRenderOptions::new()
				.render_and_save(
					vec![OHLC { o: 2.0, h: 4.0, l: 0.0, c: 1.0 }; 168],
					&Path::new("test-repetition.png")
				);
		}

		#[test]
		fn render_draw_v_axis_lines() {
			let _ = OHLCRenderOptions::new()
				.v_axis(|va| va
					.line_colour(0x000000FF)
					.line_frequency(5.)
				)
				.render_and_save(
					vec![OHLC { o: 2.0, h: 12.0, l: 0.0, c: 6.0 }; 168],
					&Path::new("test-draw-lines-vaxis.png")
				);
		}

		#[test]
		fn render_up_down() {
			let _ = OHLCRenderOptions::new()
				.render_and_save(
					vec![
						OHLC { o: 1.0, h: 4.0, l: 0.0, c: 2.0 },
						OHLC { o: 2.0, h: 4.0, l: 0.0, c: 1.0 }
					],
					&Path::new("test-up-down.png")
				);
		}

		#[test]
		fn render_temp_copy() {
			let _ = OHLCRenderOptions::new()
				.render(
					vec![OHLC { o: 2.0, h: 4.0, l: 0.0, c: 1.0 }; 3],
					|path| if let Err(err) = fs::copy(path, &Path::new("test-temp-copy.png")) {
						Err(format!("File copy error: {:?}", err))
					} else {
						Ok(())
					});
		}
	*/

	// Technically not a test, it just generates the fonts array based on the fonts png.
	#[test]
	fn generate_fonts_file() {
		let img = image::open("consolas-18px-ascii-table.png").unwrap();

		// Character sizes are 7 wide, 12 tall

		// ascii table will have 126 elements
		// First 31 elements of output array are empty

		// Printables are 0x20 - 0x7E

		let mut output = "pub const ASCII_TABLE: [[u8; 170]; 127] = [\n".to_string();

		// 0x00 to 0x20 is filled with blank
		for _ in 0..(32 + 1) {
			output += "\t[0u8; 170],\n";
		}

		for base_y in 2..8 {
			for base_x in 0..16 {
				if (base_y == 7 && base_x == 15) || (base_y == 2 && base_x == 0) { continue }
				output += "\t[\n";
				// Write character into array.
				for ptr_y in 0..17 {
					output += "\t\t";
					for ptr_x in 0..10 {
						let x = (base_x * 20) + 10 + ptr_x;
						let y = (base_y * 18) + ptr_y;

						output += &format!("{},{}", 255 - img.get_pixel(x, y).data[0], if ptr_x != 9 { " " } else { "" });
					}
					output += "\n";
				}
				output += "\t],\n";
			}
		}

		output += "];";

		let mut f = File::create("src/fonts.rs").unwrap();
		let _ = f.write(output.as_bytes());
	}
}
