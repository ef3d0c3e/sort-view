use rayon::prelude::*;
use tokio::task;
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::Semaphore;

use rand::seq::SliceRandom;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Color(u32);

impl Color {
	pub fn to_rgb(&self) -> [u8; 3] {
		[(self.0 >> 16) as u8, (self.0 >> 8) as u8, (self.0) as u8]
	}

	pub fn lerp(&self, other: &Color, f: f32) -> Color {
		let c1 = self.to_rgb();
		let c2 = other.to_rgb();
		Color(
			(((c1[0] as f32) * (1.0 - f) + (c2[0] as f32) * f) as u32) << 16
				| (((c1[1] as f32) * (1.0 - f) + (c2[1] as f32) * f) as u32) << 8
				| (((c1[2] as f32) * (1.0 - f) + (c2[2] as f32) * f) as u32),
		)
	}
}

pub struct Gradient {
	pub colors: Vec<Color>,
}

impl Gradient {
	pub fn sample(&self, v: f32) -> Color {
		if v < 0.0 {
			return self.colors[0];
		}
		if v >= 1.0 {
			return self.colors[self.colors.len() - 1];
		}

		let idx = (self.colors.len() - 1) as f32 * v;
		let c1 = self.colors[idx.floor() as usize];
		let c2 = self.colors[idx.ceil() as usize];
		c1.lerp(&c2, idx - idx.floor())
	}
}

pub struct Sort {
	pub bar_width: u32,
	pub height: u32,
	pub margin: u32,
	pub margin_top: u32,
	pub spacing: u32,
	pub bg_color: Color,
	pub colors: Gradient,
}

impl Sort {
	fn render(&self, list: &Vec<u32>, highlights: HashMap<usize, f32>) -> Vec<u8> {
		let max = list.iter().fold(u32::MIN, |r, v| r.max(*v));
		let width = self.margin * 2
			+ self.spacing * ((list.len() as u32) - 1)
			+ self.bar_width * (list.len() as u32);

		let mut buffer = Vec::new();
		let cursor = std::io::Cursor::new(&mut buffer);
		let mut enc = png::Encoder::new(cursor, width, self.height + 2 * self.margin_top);
		enc.set_color(png::ColorType::Rgb);
		enc.set_depth(png::BitDepth::Eight);
		let mut writer = enc.write_header().unwrap();

		let bg_color = self.bg_color.to_rgb();
		let mut data = Vec::new();
		data.resize(
			(width * (self.height + self.margin_top * 2) * 3) as usize,
			0,
		);
		data.par_chunks_mut(3 * width as usize)
			.enumerate()
			.for_each(|(y, row)| {
				if y < self.margin_top as usize || y >= (self.margin_top + self.height) as usize {
					for px in row.chunks_exact_mut(3) {
						px.copy_from_slice(&bg_color);
					}
				} else {
					let bar_y = (y as u32) - self.margin_top;
					let mut x = 0;

					// Left margin
					for _ in 0..self.margin {
						row[x..x + 3].copy_from_slice(&bg_color);
						x += 3;
					}

					// Bars
					for (i, &val) in list.iter().enumerate() {
						if i != 0 {
							for _ in 0..self.spacing {
								row[x..x + 3].copy_from_slice(&bg_color);
								x += 3;
							}
						}

						let h_frac = (val as f32) / (max as f32);
						let bar_height_frac = 1.0 - (bar_y as f32) / (self.height as f32);

						let color = if h_frac >= bar_height_frac {
							let factor = *highlights
								.get(&i)
								.unwrap_or(&0.0);

							self.colors
								.sample(h_frac)
								.lerp(&Color(0xFFFFFF), factor)
								.to_rgb()
						} else {
							bg_color
						};

						for _ in 0..self.bar_width {
							row[x..x + 3].copy_from_slice(&color);
							x += 3;
						}
					}

					// Right margin
					for _ in 0..self.margin {
						row[x..x + 3].copy_from_slice(&bg_color);
						x += 3;
					}
				}
			});

		writer.write_image_data(&data).unwrap();
		writer.finish().unwrap();
		buffer
	}
}

pub fn bubble_sort(sort: &Sort, mut list: Vec<u32>) {
	pub fn swap(sort: &Sort, count: &mut usize, list: &mut Vec<u32>, x: usize, y: usize) {
		let tmp = list[y];
		list[y] = list[x];
		list[x] = tmp;
		let image = sort.render(list, HashMap::default());
		std::fs::write(format!("sort-{count}.png"), image).unwrap();
		*count += 1;
	}

	let mut count = 0;
	for i in 0..list.len() {
		for j in i + 1..list.len() {
			if list[j] < list[i] {
				swap(sort, &mut count, &mut list, i, j)
			}
		}
	}
	println!("{list:#?}");
}

pub struct SortData {
	pub list: Vec<u32>,
	name: &'static str,
	count: usize,
	sort: Arc<Sort>,

	write_sem: Arc<Semaphore>,
	handles: Vec<task::JoinHandle<()>>,
	highlights: HashMap<usize, f32>,
}

impl SortData {
	pub fn new(sort: Sort, name: &'static str, list: Vec<u32>) -> Self {
		Self {
			list,
			name,
			count: 0,
			sort: Arc::new(sort),
			write_sem: Arc::new(Semaphore::new(8)),
			handles: Vec::new(),
			highlights: HashMap::new(),
		}
	}

	pub async fn finish(mut self)
	{
		for handle in self.handles.drain(..)
		{
			handle.await.unwrap();
		}
	}

	pub fn swap(&mut self, x: usize, y: usize) {
		let tmp = self.list[y];
		self.list[y] = self.list[x];
		self.list[x] = tmp;

		let sem = self.write_sem.clone();
		let list = self.list.clone();
		let sort = self.sort.clone();
		let name = self.name.clone();
		let count = self.count;
		let mut hi = self.highlights.clone();
		let handle = tokio::task::spawn(async move {
			let _permit = sem.acquire().await.unwrap();

			hi.insert(x, 0.2);
			hi.insert(y, 0.2);
			let image = sort.render(&list, hi);
			std::fs::write(format!("{}-{}.png", name, count), image).unwrap();
		});
		self.handles.push(handle);
		self.count += 1;
	}

	pub fn compare(&mut self, x: usize, y: usize) -> Ordering {
		let hi: HashSet<usize> = [x, y].into_iter().collect();

		let sem = self.write_sem.clone();
		let list = self.list.clone();
		let sort = self.sort.clone();
		let name = self.name.clone();
		let count = self.count.clone();
		let hi = self.highlights.clone();
		let handle = tokio::task::spawn(async move {
			let _permit = sem.acquire().await.unwrap();

			let image = sort.render(&list, hi);
			std::fs::write(format!("{}-{}.png", name, count), image).unwrap();
		});
		self.handles.push(handle);
		self.count += 1;
		self.list[x].cmp(&self.list[y])
	}

	pub fn set_highlight(&mut self, idx: usize, value: f32) {
		if value == 0.0
		{
			self.highlights.remove(&idx);
			return;
		}
		self.highlights.insert(idx, value);
	}
}

pub fn qsort(data: &mut SortData) {
	fn partition(data: &mut SortData, lo: usize, hi: usize) -> usize {
		let pivot = data.list[hi];
		let mut i = lo;
		data.set_highlight(hi, 0.5);

		for j in lo..hi {
			data.set_highlight(j, 0.3);
			if data.list[j] <= pivot {
				data.swap(i, j);
				i += 1;
			}
			data.set_highlight(j, 0.0);
		}

		data.set_highlight(hi, 0.0);
		data.swap(i, hi);
		i
	}

	fn qsort_recurse(data: &mut SortData, lo: usize, hi: usize) {
		if lo >= hi {
			return;
		}

		let pivot = partition(data, lo, hi);
		qsort_recurse(data, lo, pivot.saturating_sub(1));
		qsort_recurse(data, pivot + 1, hi);
	}

	let hi = data.list.len() - 1;
	qsort_recurse(data, 0, hi);
}

#[tokio::main]
async fn main() {
	let sort = Sort {
		bar_width: 8,
		height: 512,
		margin: 24,
		margin_top: 32,
		spacing: 2,
		bg_color: Color(0x1f1f1f),
		colors: Gradient {
			colors: vec![Color(0x0000FF), Color(0x70AF00), Color(0xFF0000)],
		},
	};
	let mut arr: Vec<u32> = (1..20).collect();
	arr.shuffle(&mut rand::rng());
	let mut data = SortData::new(sort, "quicksort", arr);
	qsort(&mut data);
	data.finish().await;
}
