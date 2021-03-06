use std::marker::PhantomData;

use model::*;
use model::rex::ema::median_list;

#[derive(Debug)]
struct BandPoints {
	higher: f64,
	median: f64,
	lower: f64,
}

#[derive(Clone, Debug)]
pub struct BollingerBands<C> {
	_c: PhantomData<C>,
	periods: usize,
	standard_deviations: usize,
	line_colour: u32,
}

impl<C> BollingerBands<C> {
	pub fn new(periods: usize, standard_deviations: usize, line_colour: u32) -> BollingerBands<C> {
		BollingerBands { _c: PhantomData, periods, standard_deviations, line_colour }
	}
}

impl<C: Candle> RendererExtension for BollingerBands<C> {
	type Candle = C;

	fn apply(&self, buffer: &mut ChartBuffer, data: &[C]) {
		let mut bands = vec![];

		for i in self.periods..data.len() {
			let min = i - self.periods;

			let data_slice = &data[min..i];
			let medians = median_list(data_slice);
			let scaled_std_dev = std_dev(&medians[..]) * self.standard_deviations as f64;
			let moving_avg = avg(&medians[..]);
			let points = BandPoints {
				higher: moving_avg + scaled_std_dev,
				median: moving_avg,
				lower: moving_avg - scaled_std_dev,
			};

			bands.push(points);
		}

		let offset = ((self.periods as f64 + 0.5) * (buffer.timeframe as f64) / (data.len() as f64)) as i64;

		for i in 0..(bands.len() - 1) {
			let time = (i as i64 * buffer.timeframe / data.len() as i64) as i64 + offset;
			let time_next_period = ((i as i64 + 1) * buffer.timeframe / data.len() as i64) as i64 + offset;

			let p1_h = buffer.data_to_coords(bands[i].higher, time);
			let p2_h = buffer.data_to_coords(bands[i + 1].higher, time_next_period);

			buffer.line(p1_h, p2_h, self.line_colour);

			let p1_m = buffer.data_to_coords(bands[i].median, time);
			let p2_m = buffer.data_to_coords(bands[i + 1].median, time_next_period);

			buffer.line(p1_m, p2_m, self.line_colour);

			let p1_l = buffer.data_to_coords(bands[i].lower, time);
			let p2_l = buffer.data_to_coords(bands[i + 1].lower, time_next_period);

			buffer.line(p1_l, p2_l, self.line_colour);
		}
	}

	fn lore_colour(&self) -> Option<u32> {
		Some(self.line_colour)
	}

	fn name(&self) -> String {
		format!("BB({}, {})", self.periods, self.standard_deviations)
	}
}

fn std_dev(prices: &[f64]) -> f64 {
	let len = prices.len();
	if len <= 1 {
		return 0.;
	}

	let avg = avg(prices);
	let mut squared_diff_sum = 0.;

	for price in prices {
		squared_diff_sum += (avg - price).powf(2.);
	}

	(squared_diff_sum / (len - 1) as f64).sqrt()
}

fn avg(prices: &[f64]) -> f64 {
	let mut sum = 0.;

	for price in prices {
		sum += *price;
	}

	sum / prices.len() as f64
}
