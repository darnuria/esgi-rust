extern crate crossbeam;
extern crate image;
extern crate num;

use num::Complex;
use std::str::FromStr;

use crate::image::ImageEncoder;
use image::codecs::png::PngEncoder;
use image::ColorType;
use std::fs::File;

fn escape_time(c: Complex<f64>, limit: u32) -> Option<u32> {
    let mut z = Complex { re: 0.0, im: 0.0 };
    for i in 0..limit {
        z = z * z + c;
        if z.norm_sqr() > 4.0 {
            return Some(i);
        }
    }

    None
}

/// Parse a string `s` into a coordinate pair. Like "200x600" or "20.10,0.0"
/// Secificially, `s` should have a form <left><separator><right>, where <sep> is the
/// caracter given by  `separator` argument, and <left> and <right>
/// the string parsed by `T::from_str.`
fn parse_pair<T: FromStr>(s: &str, separator: char) -> Option<(T, T)> {
    let index = s.find(separator)?;
    match (T::from_str(&s[..index]), T::from_str(&s[index + 1..])) {
        (Ok(l), Ok(r)) => Some((l, r)),
        _ => None,
    }
}

/// Compute a string including two floating point number separated by a
/// comma into a complex number.
/// exemple:
/// ```rust
/// assert_eq!(parse_complex("10.10, -0.1"), Some(Complexe { re: 10.10, im: -0.1 });
/// ```
fn parse_complex(s: &str) -> Option<Complex<f64>> {
    let (re, im) = parse_pair(s, ',')?;
    Some(Complex { re, im })
}

/// Given the row and collumn of a pixel in the output image
/// return the corresponding point on the complex plane.
///
/// `bound` is a pair giving the idth and the height of the image i pixels.
/// `pixel is a (column, row) pair indicating a particular pixel in that image,
/// The `upper_left` and `lower_right` parameters are points on the complex plane,
/// designating the area our image covers.
fn pixel_to_point(
    bounds: (usize, usize),
    pixel: (usize, usize),
    upper_left: Complex<f64>,
    lower_right: Complex<f64>,
) -> Complex<f64> {
    let (width, height) = (
        lower_right.re - upper_left.re,
        upper_left.im - lower_right.im,
    );

    Complex {
        re: upper_left.re + pixel.0 as f64 * width / bounds.0 as f64,
        im: upper_left.im - pixel.1 as f64 * height / bounds.1 as f64,
        // Substracting because pixel.1 increase as we go down.
        // But immaginary increase as we go up.
    }
}

fn render(
    pixels: &mut [u8],
    bounds: (usize, usize),
    upper_left: Complex<f64>,
    lower_right: Complex<f64>,
) {
    assert!(pixels.len() == bounds.0 * bounds.1);

    for row in 0..bounds.1 {
        for column in 0..bounds.0 {
            let point = pixel_to_point(bounds, (column, row), upper_left, lower_right);

            pixels[row * bounds.0 + column] = match escape_time(point, 225) {
                None => 0,
                Some(count) => 255 - count as u8,
            }
        }
    }
}

use std::error::Error;
/// Write the buffer `pixels`, whose dimensions are given by `bounds`, to the file named `filename.
fn write_image(
    filename: &str,
    pixels: &[u8],
    bounds: (usize, usize),
) -> Result<(), Box<dyn Error>> {
    let output = File::create(filename)?;

    let encoder = PngEncoder::new(output);
    encoder.write_image(&pixels, bounds.0 as u32, bounds.1 as u32, ColorType::L8)?;

    Ok(())
}

use std::io::Write;

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.len() != 5 {
        writeln!(
            std::io::stderr(),
            "Usage: mandelbrot FILE PIXELS UPPERLEFT LOWERRIGHT\n
        Example: {} mandel.png 100x750 -1.20,0.35 -1,0.20",
            args[0]
        )
        .unwrap();
        std::process::exit(1);
    }

    let bounds = parse_pair(&args[2], 'x').expect("error parsing image dimensions");
    let upper_left = parse_complex(&args[3]).expect("error parsing upper left conner point");
    let lower_right = parse_complex(&args[4]).expect("error parsing lower right conner point");
    let filename = &args[1];

    let mut pixels = vec![0; bounds.0 * bounds.1];

    let threads = 8;
    let rows_per_band = bounds.1 / threads + 1;
    {
        let bands: Vec<&mut [u8]> = pixels.chunks_mut(rows_per_band * bounds.0).collect();
        crossbeam::scope(|scope| {
            for (i, band) in bands.into_iter().enumerate() {
                let top = rows_per_band * i;
                let height = band.len() / bounds.0;
                let band_bounds = (bounds.0, height);
                let band_upper_left = pixel_to_point(bounds, (0, top), upper_left, lower_right);
                let band_lower_right =
                    pixel_to_point(bounds, (bounds.0, top + height), upper_left, lower_right);
                scope.spawn(move |_| {
                    render(band, band_bounds, band_upper_left, band_lower_right);
                });
            }
        })
        .expect("Failed to start thread!");
    };

    write_image(filename, &pixels, bounds).expect("error writing PNG file");
}

#[test]
fn test_parse_pair() {
    assert_eq!(parse_pair::<i32>("", ','), None);
    assert_eq!(parse_pair::<i32>("10,", ','), None);
    assert_eq!(parse_pair::<i32>("*10", '*'), None);
    assert_eq!(parse_pair::<i32>("t10.10xy", ','), None);
    assert_eq!(parse_pair::<f64>("0.5x", 'x'), None);
    assert_eq!(parse_pair::<i32>("10*10", '*'), Some((10, 10)));
    assert_eq!(parse_pair::<f64>("0.5x1.5", 'x'), Some((0.5, 1.5)));
    assert_eq!(parse_pair::<i32>("100x750", 'x'), Some((100, 750)));
    assert_eq!(parse_pair::<f64>("-1.20,0.35", ','), Some((-1.20, 0.35)));
    assert_eq!(parse_pair::<f64>("-1,0.20", ','), Some((-1.0, 0.20)));
}

#[test]
fn test_parse_complex() {
    assert_eq!(parse_complex("toto"), None);
    assert_eq!(
        parse_complex("1.1,-10.1"),
        Some(Complex { re: 1.1, im: -10.1 })
    );
    assert_eq!(parse_complex(",0.1"), None);
}

#[test]
fn test_pixel_to_point() {
    assert_eq!(
        pixel_to_point(
            (100, 100),
            (25, 75),
            Complex { re: -1.0, im: 1.0 },
            Complex { re: 1.0, im: -1.0 }
        ),
        Complex { re: -0.5, im: -0.5 }
    );
}
