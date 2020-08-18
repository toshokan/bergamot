use bergamot::{create_output_windows, get_connection, get_rectangles, get_screen};
use bergamot::{error::Error, Align, Area, Colour, Paint};

fn main() -> Result<(), Error> {
    let bar_height = 16;
    let font = "monospace 9";

    let conn = get_connection()?;
    let screen = get_screen(&conn);
    let rectangles = get_rectangles(&conn, &screen)?;
    let windows = create_output_windows(&conn, &screen, bar_height, rectangles);
    conn.flush();

    let mut area_paints = vec![];

    while let Some(event) = conn.wait_for_event() {
        match event.response_type() & !0x80 {
            xcb::EXPOSE => {
                let font = pango::FontDescription::from_string(font);

                for output in &windows {
                    output.ctx.set_source_rgb(0.1, 0.1, 0.1);
                    output.ctx.rectangle(
                        0.0,
                        0.0,
                        output.rect.width.into(),
                        output.rect.height.into(),
                    );
                    output.ctx.fill();

                    let mut areas = vec![
                        Area {
                            align: Align::Left,
                            tag: "left".to_string(),
                            text: "clicky_left".to_string(),
                            fg: Colour {
                                red: 1.0,
                                green: 0.0,
                                blue: 0.0,
                            },
                            bg: None,
                            onclick: None,
                        },
                        Area {
                            align: Align::Right,
                            tag: "left".to_string(),
                            text: "clicky_right".to_string(),
                            fg: Colour {
                                red: 0.0,
                                green: 0.0,
                                blue: 1.0,
                            },
                            bg: None,
                            onclick: None,
                        },
                    ];

                    let mut left_finger = 0_f64;
                    let mut right_finger = output.rect.width.into();
                    for area in areas.drain(..) {
                        let layout = pangocairo::create_layout(&output.ctx)
                            .expect("Failed to create pangocairo layout");
                        layout.set_font_description(Some(&font));
                        layout.set_text(&area.text);

                        let (w, h) = layout.get_pixel_size();
                        let area_width: f64 = (w + 10).into();
                        let layout_height: f64 = h.into();

                        let bg = area.bg.as_ref().unwrap_or(&Colour {
                            red: 0_f64,
                            blue: 0_f64,
                            green: 0_f64,
                        });

                        output.ctx.set_source_rgb(bg.red, bg.green, bg.blue);
                        let (left, right) = match area.align {
                            Align::Left => {
                                let (left, right) = (left_finger, left_finger + area_width);
                                left_finger += area_width;
                                (left, right)
                            }
                            Align::Right => {
                                let (left, right) = (right_finger - area_width, right_finger);
                                right_finger -= area_width;
                                (left, right)
                            }
                        };

                        let height = bar_height.into();

                        output.ctx.rectangle(left, 0_f64, right - left, height);
                        output.ctx.fill();

                        output
                            .ctx
                            .set_source_rgb(area.fg.red, area.fg.green, area.fg.blue);
                        output
                            .ctx
                            .move_to(left + 5_f64, height / 2_f64 - layout_height / 2_f64);
                        pangocairo::show_layout(&output.ctx, &layout);

                        area_paints.push(Paint { left, right, area })
                    }
                }
            }
            _ => (),
        }
        conn.flush();
    }

    Ok(())
}
