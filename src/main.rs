use rand::seq::SliceRandom;


#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Color(u32);

impl Color {
	pub fn to_rgb(&self) -> [u8; 3]
	{
		[(self.0 >> 16) as u8, (self.0 >> 8) as u8, (self.0) as u8]
	}
}

pub struct Gradient
{
	pub colors: Vec<Color>
}

impl Gradient {
	pub fn sample(&self, v: f32) -> Color
	{
		if v < 0.0 { return self.colors[0] }
		if v >= 1.0 { return self.colors[self.colors.len() - 1] }

		let idx = (self.colors.len() - 1) as f32 * v;
		let c1 = self.colors[idx.floor() as usize].to_rgb();
		let c2 = self.colors[idx.ceil() as usize].to_rgb();
		let f = idx - idx.floor();

		Color(
			(((c1[0] as f32) * (1.0 - f) + (c2[0] as f32) * f) as u32) << 16 |
			(((c1[1] as f32) * (1.0 - f) + (c2[1] as f32) * f) as u32) << 8 |
			(((c1[2] as f32) * (1.0 - f) + (c2[2] as f32) * f) as u32))
	}
}

pub struct Sort
{
	pub bar_width: u32,
	pub height: u32,
	pub margin: u32,
	pub margin_top: u32,
	pub spacing: u32,
	pub bg_color: Color,
	pub colors: Gradient,
}

impl Sort
{
	fn render(&self, list: &Vec<u32>) -> Vec<u8>
	{
		let max = list.iter().fold(u32::MIN, |r, v| r.max(*v));
		let width = self.margin * 2 + self.spacing * ((list.len() as u32) - 1) + self.bar_width * (list.len() as u32);

		let mut buffer = Vec::new();
		let cursor = std::io::Cursor::new(&mut buffer);
		let mut enc = png::Encoder::new(cursor, width, self.height + 2 * self.margin_top);
		enc.set_color(png::ColorType::Rgb);
		enc.set_depth(png::BitDepth::Eight);
		let mut writer = enc.write_header().unwrap();

		let bg_color = self.bg_color.to_rgb();
		let mut data = Vec::with_capacity((width * self.height * 3) as usize);
		// Top margin
		(0..self.margin_top*width).for_each(|_| data.extend_from_slice(&bg_color));	

		for y in 0..self.height
		{
			// Left margin
			(0..self.margin).for_each(|_| data.extend_from_slice(&bg_color));	

			// Bars
			for i in 0..(list.len() as u32)
			{
				// Spacing
				if i != 0 {
					(0..self.spacing).for_each(|_| data.extend_from_slice(&bg_color));	
				}

				if (list[i as usize] as f32) / (max as f32) >= 1.0 - (y as f32) / (self.height as f32)
				{
					let color = self.colors.sample((list[i as usize] as f32) / (max as f32)).to_rgb();
					(0..self.bar_width).for_each(|_| data.extend_from_slice(&color));	
				}
				else
				{
					(0..self.bar_width).for_each(|_| data.extend_from_slice(&bg_color));	
				}
			}

			// Right margin
			(0..self.margin).for_each(|_| data.extend_from_slice(&bg_color));	
		}
		// Bottom margin
		(0..self.margin_top*width).for_each(|_| data.extend_from_slice(&bg_color));	
		
		writer.write_image_data(&data).unwrap();
		writer.finish().unwrap();
		buffer
	}
}


pub fn sort(sort: &Sort, mut list: Vec<u32>)
{
	pub fn swap(sort: &Sort, count: &mut usize, list: &mut Vec<u32>, x: usize, y: usize)
	{
		let tmp = list[y];
		list[y] = list[x];
		list[x] = tmp;
		let image = sort.render(list);
		std::fs::write(format!("sort-{count}.png"), image).unwrap();
		*count += 1;
	}

	let mut count = 0;
	for i in 0..list.len()
	{
		for j in i+1..list.len()
		{
			if list[j] < list[i] { swap(sort, &mut count, &mut list, i, j) }
		}
	}
	println!("{list:#?}");
}

fn main() {
	let s = Sort {
		bar_width: 8,
		height: 512,
		margin: 24,
		margin_top: 32,
		spacing: 2,
		bg_color: Color(0x1f1f1f),
		colors: Gradient {
			colors: vec![Color(0x0000FF), Color(0x70AF00), Color(0xFF0000)],
		}
	};
	let mut arr : Vec<u32> = (1..20).collect();
	arr.shuffle(&mut rand::rng());
	sort(&s, arr);
}
