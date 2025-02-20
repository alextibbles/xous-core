use crate::api::{Circle, DrawStyle, Line, Pixel, PixelColor, Point, Rectangle, RoundedRectangle};

/// LCD Frame buffer bounds
pub const LCD_WORDS_PER_LINE: usize = 11;
pub const LCD_PX_PER_LINE: usize = 336;
pub const LCD_LINES: usize = 536;
pub const LCD_FRAME_BUF_SIZE: usize = LCD_WORDS_PER_LINE * LCD_LINES;

pub const WIDTH: i16 = 336;
pub const HEIGHT: i16 = 536;

/// For passing frame buffer references
pub type LcdFB = [u32; LCD_FRAME_BUF_SIZE];

fn put_pixel(fb: &mut LcdFB, x: i16, y: i16, color: PixelColor) {
    let mut clip_y: usize = y as usize;
    if clip_y >= LCD_LINES {
        clip_y = LCD_LINES - 1;
    }

    let clip_x: usize = x as usize;
    if clip_x >= LCD_PX_PER_LINE {
        clip_y = LCD_PX_PER_LINE - 1;
    }

    if color == PixelColor::Light {
        fb[(clip_x + clip_y * LCD_WORDS_PER_LINE * 32) / 32] |= 1 << (clip_x % 32)
    } else {
        fb[(clip_x + clip_y * LCD_WORDS_PER_LINE * 32) / 32] &= !(1 << (clip_x % 32))
    }
    // set the dirty bit on the line that contains the pixel
    fb[clip_y * LCD_WORDS_PER_LINE + (LCD_WORDS_PER_LINE - 1)] |= 0x1_0000;
}

fn xor_pixel(fb: &mut LcdFB, x: i16, y: i16) {
    let mut clip_y: usize = y as usize;
    if clip_y >= LCD_LINES {
        clip_y = LCD_LINES - 1;
    }

    let clip_x: usize = x as usize;
    if clip_x >= LCD_PX_PER_LINE {
        clip_y = LCD_PX_PER_LINE - 1;
    }

    fb[(clip_x + clip_y * LCD_WORDS_PER_LINE * 32) / 32] ^= 1 << (clip_x % 32);
    // set the dirty bit on the line that contains the pixel
    fb[clip_y * LCD_WORDS_PER_LINE + (LCD_WORDS_PER_LINE - 1)] |= 0x1_0000;
}

pub fn line(fb: &mut LcdFB, l: Line, clip: Option<Rectangle>, xor: bool) {
    let color: PixelColor;
    if l.style.stroke_color.is_some() {
        color = l.style.stroke_color.unwrap();
    } else {
        return;
    }
    let mut x0 = l.start.x;
    let mut y0 = l.start.y;
    let x1 = l.end.x;
    let y1 = l.end.y;

    let dx = (x1 - x0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let dy = -((y1 - y0).abs());
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy; /* error value e_xy */
    loop {
        /* loop */
        if x0 >= 0 && y0 >= 0 && x0 < (WIDTH as _) && y0 < (HEIGHT as _) {
            if clip.is_none() || (clip.unwrap().intersects_point(Point::new(x0, y0))) {
                if !xor {
                    put_pixel(fb, x0 as _, y0 as _, color);
                } else {
                    xor_pixel(fb, x0 as _, y0 as _);
                }
            }
        }
        if x0 == x1 && y0 == y1 {
            break;
        }
        let e2 = 2 * err;
        if e2 >= dy {
            /* e_xy+e_x > 0 */
            err += dy;
            x0 += sx;
        }
        if e2 <= dx {
            /* e_xy+e_y < 0 */
            err += dx;
            y0 += sy;
        }
    }
}

/// Pixel iterator for each pixel in the circle border
/// lifted from embedded-graphics crate
#[derive(Debug, Copy, Clone)]
pub struct CircleIterator {
    center: Point,
    radius: u16,
    style: DrawStyle,
    p: Point,
    clip: Option<Rectangle>,
}

impl Iterator for CircleIterator {
    type Item = Pixel;

    // https://stackoverflow.com/questions/1201200/fast-algorithm-for-drawing-filled-circles
    fn next(&mut self) -> Option<Self::Item> {
        // If border or stroke colour is `None`, treat entire object as transparent and exit early
        if self.style.stroke_color.is_none() && self.style.fill_color.is_none() {
            return None;
        }

        let radius = self.radius as i16 - self.style.stroke_width + 1;
        let outer_radius = self.radius as i16;

        let radius_sq = radius * radius;
        let outer_radius_sq = outer_radius * outer_radius;

        loop {
            let mut item = None;

            if self.clip.is_none() || // short-circuit evaluation makes this safe
               (self.clip.unwrap().intersects_point(self.p + self.center))
            {
                let t = self.p;
                let len = t.x * t.x + t.y * t.y;

                let is_border = len > (radius_sq - radius) && len < (outer_radius_sq + radius);

                let is_fill = len <= outer_radius_sq + 1;

                item = if is_border && self.style.stroke_color.is_some() {
                    Some(Pixel(
                        self.center + t,
                        self.style.stroke_color.expect("Border color not defined"),
                    ))
                } else if is_fill && self.style.fill_color.is_some() {
                    Some(Pixel(
                        self.center + t,
                        self.style.fill_color.expect("Fill color not defined"),
                    ))
                } else {
                    None
                };
            }

            self.p.x += 1;

            if self.p.x > self.radius as i16 {
                self.p.x = -(self.radius as i16);
                self.p.y += 1;
            }

            if self.p.y > self.radius as i16 {
                break None;
            }

            if item.is_some() {
                break item;
            }
        }
    }
}

pub fn circle(fb: &mut LcdFB, circle: Circle, clip: Option<Rectangle>) {
    let radius = circle.radius.abs() as u16;
    let c = CircleIterator {
        center: circle.center,
        radius,
        style: circle.style,
        p: Point::new(-(radius as i16), -(radius as i16)),
        clip,
    };

    for pixel in c {
        put_pixel(fb, pixel.0.x, pixel.0.y, pixel.1);
    }
}

/// Pixel iterator for each pixel in the rect border
/// lifted from embedded-graphics crate
#[derive(Debug, Clone, Copy)]
pub struct RectangleIterator {
    top_left: Point,
    bottom_right: Point,
    style: DrawStyle,
    p: Point,
    clip: Option<Rectangle>,
}

impl Iterator for RectangleIterator {
    type Item = Pixel;

    fn next(&mut self) -> Option<Self::Item> {
        // Don't render anything if the rectangle has no border or fill color.
        if self.style.stroke_color.is_none() && self.style.fill_color.is_none() {
            return None;
        }

        loop {
            let mut out = None;

            // Finished, i.e. we're below the rect
            if self.p.y > self.bottom_right.y {
                break None;
            }

            if self.clip.is_none() || // short-circuit evaluation makes this safe
               (self.clip.unwrap().intersects_point(self.p))
            {
                let border_width = self.style.stroke_width;
                let tl = self.top_left;
                let br = self.bottom_right;

                // Border
                if (
                    // Top border
                    (self.p.y >= tl.y && self.p.y < tl.y + border_width)
                // Bottom border
                || (self.p.y <= br.y && self.p.y > br.y - border_width)
                // Left border
                || (self.p.x >= tl.x && self.p.x < tl.x + border_width)
                // Right border
                || (self.p.x <= br.x && self.p.x > br.x - border_width)
                ) && self.style.stroke_color.is_some()
                {
                    out = Some(Pixel(
                        self.p,
                        self.style.stroke_color.expect("Expected stroke"),
                    ));
                }
                // Fill
                else if let Some(fill) = self.style.fill_color {
                    out = Some(Pixel(self.p, fill));
                }
            }

            self.p.x += 1;

            // Reached end of row? Jump down one line
            if self.p.x > self.bottom_right.x {
                self.p.x = self.top_left.x;
                self.p.y += 1;
            }

            if out.is_some() {
                break out;
            }
        }
    }
}

pub fn rectangle(fb: &mut LcdFB, rect: Rectangle, clip: Option<Rectangle>) {
    let r = RectangleIterator {
        top_left: rect.tl,
        bottom_right: rect.br,
        style: rect.style,
        p: rect.tl,
        clip: clip,
    };

    for pixel in r {
        put_pixel(fb, pixel.0.x, pixel.0.y, pixel.1);
    }
}

/////////////////////////////////////////////////// rounded rectangle

#[derive(Debug, Clone, Copy)]
pub enum Quadrant {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

#[derive(Debug, Copy, Clone)]
pub struct QuadrantIterator {
    center: Point,
    radius: u16,
    style: DrawStyle,
    p: Point,
    quad: Quadrant,
    clip: Option<Rectangle>,
}

impl Iterator for QuadrantIterator {
    type Item = Pixel;

    // https://stackoverflow.com/questions/1201200/fast-algorithm-for-drawing-filled-circles
    fn next(&mut self) -> Option<Self::Item> {
        // If border and stroke colour is `None`, treat entire object as transparent and exit early
        if self.style.stroke_color.is_none() && self.style.fill_color.is_none() {
            return None;
        }

        let inner_radius = self.radius as i16 - self.style.stroke_width + 1;
        let outer_radius = self.radius as i16;

        let inner_radius_sq = inner_radius * inner_radius;
        let outer_radius_sq = outer_radius * outer_radius;

        //log::info!("GFX|OP: quaditerator {}, {:?}, {:?}, {:?}, {:?}, {:?}", self.radius, self.center, self.p, self.quad, self.clip, self.style);
        loop {
            let mut item = None;
            if self.clip.is_none() || // short-circuit evaluation makes this safe
               (self.clip.unwrap().intersects_point(self.p + self.center))
            {
                let t = self.p;
                let len = t.x * t.x + t.y * t.y;

                let is_border = len > (inner_radius_sq - inner_radius)
                    && len < (outer_radius_sq + inner_radius);

                let is_fill = len <= outer_radius_sq + 1;

                item = if is_border && self.style.stroke_color.is_some() {
                    Some(Pixel(
                        self.center + t,
                        self.style.stroke_color.expect("Border color not defined"),
                    ))
                } else if is_fill && self.style.fill_color.is_some() {
                    Some(Pixel(
                        self.center + t,
                        self.style.fill_color.expect("Fill color not defined"),
                    ))
                } else {
                    None
                };
            }

            self.p.x += 1;

            match self.quad {
                Quadrant::TopLeft => {
                    if self.p.x > 0 as i16 {
                        self.p.x = -(self.radius as i16);
                        self.p.y += 1;
                    }
                    if self.p.y > 1 as i16 {
                        break None;
                    }
                    if item.is_some() {
                        break item;
                    }
                }
                Quadrant::TopRight => {
                    if self.p.x > self.radius as i16 {
                        self.p.x = 0;
                        self.p.y += 1;
                    }
                    if self.p.y > 1 as i16 {
                        break None;
                    }
                    if item.is_some() {
                        break item;
                    }
                }
                Quadrant::BottomLeft => {
                    if self.p.x > 0 as i16 {
                        self.p.x = -(self.radius as i16);
                        self.p.y += 1;
                    }
                    if self.p.y > self.radius as i16 {
                        break item;
                    }
                    if item.is_some() {
                        break item;
                    }
                }
                Quadrant::BottomRight => {
                    if self.p.x > self.radius as i16 {
                        self.p.x = 0;
                        self.p.y += 1;
                    }
                    if self.p.y > self.radius as i16 {
                        break item;
                    }
                    if item.is_some() {
                        break item;
                    }
                }
            }
        }
    }
}

pub fn quadrant(fb: &mut LcdFB, circle: Circle, quad: Quadrant, clip: Option<Rectangle>) {
    let starting_pixel = match quad {
        Quadrant::TopLeft => Point::new(-(circle.radius as i16), -(circle.radius as i16)),
        Quadrant::TopRight => Point::new(0, -(circle.radius as i16)),
        Quadrant::BottomLeft => Point::new(-(circle.radius as i16), 0),
        Quadrant::BottomRight => Point::new(0, 0),
    };
    let q = QuadrantIterator {
        center: circle.center,
        radius: circle.radius as _,
        style: circle.style,
        p: starting_pixel,
        quad: quad,
        clip: clip,
    };

    for pixel in q {
        put_pixel(fb, pixel.0.x, pixel.0.y, pixel.1);
    }
}

/// Pixel iterator for each pixel in the rect border
/// lifted from embedded-graphics crate
#[derive(Debug, Clone, Copy)]
pub struct RoundedRectangleIterator {
    top_left: Point,
    bottom_right: Point,
    style: DrawStyle,
    p: Point,
    clip: Option<Rectangle>,
    // the four quadrants for drawing the rounded corners
    tlq: Rectangle,
    trq: Rectangle,
    blq: Rectangle,
    brq: Rectangle,
}

impl Iterator for RoundedRectangleIterator {
    type Item = Pixel;

    fn next(&mut self) -> Option<Self::Item> {
        // Don't render anything if the rectangle has no border or fill color.
        if self.style.stroke_color.is_none() && self.style.fill_color.is_none() {
            return None;
        }
        loop {
            let mut out = None;

            // Finished, i.e. we're below the rect
            if self.p.y > self.bottom_right.y {
                break None;
            }

            if self.clip.is_none() || // short-circuit evaluation makes this safe
               (self.clip.unwrap().intersects_point(self.p))
            {
                // suppress the output pixel if we happen to be in the corner quadrant area
                if self.tlq.intersects_point(self.p)
                    || self.trq.intersects_point(self.p)
                    || self.blq.intersects_point(self.p)
                    || self.brq.intersects_point(self.p)
                {
                    out = None
                } else {
                    let border_width = self.style.stroke_width;
                    let tl = self.top_left;
                    let br = self.bottom_right;

                    // Border
                    if (
                        // Top border
                        (self.p.y >= tl.y && self.p.y < tl.y + border_width)
                            // Bottom border
                            || (self.p.y <= br.y && self.p.y > br.y - border_width)
                            // Left border
                            || (self.p.x >= tl.x && self.p.x < tl.x + border_width)
                            // Right border
                            || (self.p.x <= br.x && self.p.x > br.x - border_width)
                    ) && self.style.stroke_color.is_some()
                    {
                        out = Some(Pixel(
                            self.p,
                            self.style.stroke_color.expect("Expected stroke"),
                        ));
                    }
                    // Fill
                    else if let Some(fill) = self.style.fill_color {
                        out = Some(Pixel(self.p, fill));
                    }
                }
            }

            self.p.x += 1;

            // Reached end of row? Jump down one line
            if self.p.x > self.bottom_right.x {
                self.p.x = self.top_left.x;
                self.p.y += 1;
            }

            if out.is_some() {
                break out;
            }
        }
    }
}

pub fn rounded_rectangle(fb: &mut LcdFB, rr: RoundedRectangle, clip: Option<Rectangle>) {
    // compute the four quadrants
    // call the rr iterator on the rectangle
    // then call it on one each of the four circle quadrants

    //log::info!("GFX|OP: rr: {:?}, clip: {:?}", rr, clip);
    let rri = RoundedRectangleIterator {
        top_left: rr.border.tl,
        bottom_right: rr.border.br,
        style: rr.border.style,
        p: rr.border.tl,
        clip,
        tlq: Rectangle::new(
            rr.border.tl,
            Point::new(rr.border.tl.x + rr.radius, rr.border.tl.y + rr.radius),
        ),
        trq: Rectangle::new(
            Point::new(rr.border.br.x - rr.radius, rr.border.tl.y),
            Point::new(rr.border.br.x, rr.border.tl.y + rr.radius),
        ),
        blq: Rectangle::new(
            Point::new(rr.border.tl.x, rr.border.br.y - rr.radius),
            Point::new(rr.border.tl.x + rr.radius, rr.border.br.y),
        ),
        brq: Rectangle::new(
            Point::new(rr.border.br.x - rr.radius, rr.border.br.y - rr.radius),
            rr.border.br,
        ),
    };
    // draw the body
    for pixel in rri {
        put_pixel(fb, pixel.0.x, pixel.0.y, pixel.1);
    }
    //log::info!("GFX|OP: topleft {:?}, {:?}, {:?}, {:?}", rri.tlq.br, rr.radius, rr.border.style, clip);
    // now draw the corners
    quadrant(
        fb,
        Circle::new_with_style(rri.tlq.br, rr.radius, rr.border.style),
        Quadrant::TopLeft,
        clip,
    );
    quadrant(
        fb,
        Circle::new_with_style(
            Point::new(rri.trq.tl.x, rri.trq.br.y),
            rr.radius,
            rr.border.style,
        ),
        Quadrant::TopRight,
        clip,
    );
    quadrant(
        fb,
        Circle::new_with_style(
            Point::new(rri.blq.br.x, rri.blq.tl.y),
            rr.radius,
            rr.border.style,
        ),
        Quadrant::BottomLeft,
        clip,
    );
    quadrant(
        fb,
        Circle::new_with_style(rri.brq.tl, rr.radius, rr.border.style),
        Quadrant::BottomRight,
        clip,
    );
}
