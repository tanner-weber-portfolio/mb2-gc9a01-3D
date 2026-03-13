#![no_main]
#![no_std]

use cortex_m_rt::entry;
use embedded_graphics::{
    Drawable,
    pixelcolor::Rgb565,
    prelude::*,
    primitives::{Line, PrimitiveStyle},
};
use embedded_hal_bus::spi::ExclusiveDevice;
use microbit::hal::{
    Spim,
    gpio::Level,
    saadc,
    spim::{self, Frequency},
    timer::Timer,
};
use mipidsi::{
    Builder,
    models::GC9A01,
    options::{ColorInversion, Orientation, Rotation},
};
use nalgebra::{Rotation3, Vector3};
use panic_rtt_target as _;
use rtt_target::rtt_init_print;

const POT_PIN_MAX_READ: i16 = 16_000;
const EDGE_COUNT: usize = 8;
const VERT_COUNT: usize = 5;
const STROKE_WIDTH: u32 = 3;

#[entry]
fn main() -> ! {
    rtt_init_print!();

    let board = microbit::Board::take().unwrap();
    let mut timer0 = Timer::new(board.TIMER0);

    // Setup SPI
    let sck = board.pins.p0_17.into_push_pull_output(Level::Low).degrade();
    let coti = board.pins.p0_13.into_push_pull_output(Level::Low).degrade();

    let dc = board.edge.e08.into_push_pull_output(Level::Low);
    let cs = board.edge.e01.into_push_pull_output(Level::Low);
    let rst = board.edge.e09.into_push_pull_output(Level::High);

    let spi_bus = Spim::new(
        board.SPIM3,
        microbit::hal::spim::Pins {
            sck: Some(sck),
            mosi: Some(coti),
            miso: None,
        },
        Frequency::M32,
        spim::MODE_0,
        0xFF, // ORC overflow character
    );
    let spi = display_interface_spi::SPIInterface::new(
        ExclusiveDevice::new_no_delay(spi_bus, cs).unwrap(),
        dc,
    );

    // Setup GC9A01 display using mipidsi
    let mut display = Builder::new(GC9A01, spi)
        .orientation(Orientation::new().rotate(Rotation::Deg180))
        .invert_colors(ColorInversion::Inverted)
        .reset_pin(rst)
        .init(&mut timer0)
        .unwrap();

    // Call `embedded_graphics` `clear()` trait method
    <_ as embedded_graphics::draw_target::DrawTarget>::clear(
        &mut display,
        Rgb565::WHITE,
    )
    .unwrap();

    // Set up potentiometer.
    let mut pot_pin = board.edge.e02.into_floating_input();
    let saadc_config = saadc::SaadcConfig::default();
    let mut saadc = saadc::Saadc::new(board.ADC, saadc_config);

    // Vertices for a tetrahedron.
    let vertices3d: [Vector3<f32>; VERT_COUNT] = [
        Vector3::new(0.0f32, 10.0f32, 0.0f32),
        Vector3::new(10.0f32, -10.0f32, 10.0f32),
        Vector3::new(-10.0f32, -10.0f32, 10.0f32),
        Vector3::new(-10.0f32, -10.0f32, -10.0f32),
        Vector3::new(10.0f32, -10.0f32, -10.0f32),
    ];

    // Edges on the tetrahedron corresponding to points in the array.
    let edges: [(usize, usize); EDGE_COUNT] = [
        (0, 1),
        (0, 2),
        (0, 3),
        (0, 4),
        (1, 2),
        (2, 3),
        (3, 4),
        (4, 1),
    ];
    let edge_colors: [Rgb565; EDGE_COUNT] = [
        Rgb565::RED,
        Rgb565::GREEN,
        Rgb565::BLUE,
        Rgb565::YELLOW,
        Rgb565::CSS_VIOLET,
        Rgb565::CSS_PINK,
        Rgb565::CSS_BROWN,
        Rgb565::CSS_DARK_GRAY,
    ];

    // The 2D points to draw edges between.
    let mut points: [Point; VERT_COUNT] = [
        Point::new(0, 0),
        Point::new(0, 0),
        Point::new(0, 0),
        Point::new(0, 0),
        Point::new(0, 0),
    ];

    let camera_pos = Vector3::<f32>::new(30.0, 5.0, 40.0);
    let display_surface = Vector3::<f32>::new(0.0, 0.0, 20.0);

    loop {
        let saadc_result = saadc.read_channel(&mut pot_pin).unwrap();
        let new_rot = scale_saadc_result(saadc_result);
        let rotation = Vector3::<f32>::new(-0.2, 0.5, new_rot);

        for (i, v) in vertices3d.iter().enumerate() {
            points[i] = convert_vertex_to_2d_point(
                v,
                &rotation,
                &camera_pos,
                &display_surface,
            );
        }

        convert_points_to_display_coords(&mut points);

        display.clear(Rgb565::BLACK).unwrap();

        for (i, edge) in edges.iter().enumerate() {
            Line::new(
                Point::new(points[edge.0].x, points[edge.0].y),
                Point::new(points[edge.1].x, points[edge.1].y),
            )
            .into_styled(PrimitiveStyle::with_stroke(
                edge_colors[i],
                STROKE_WIDTH,
            ))
            .draw(&mut display)
            .unwrap();
        }
    }
}

/// Converts to display coords which range from 0 to 240 on each axis.
fn convert_points_to_display_coords(points: &mut [Point]) {
    for p in points {
        *p = Point::new(p.x + 119, p.y + 119);
    }
}

/// Projects a 3D vertex to a 2D point.
fn convert_vertex_to_2d_point(
    vec: &Vector3<f32>,
    rotation: &Vector3<f32>,
    cam_pos: &Vector3<f32>,
    surf: &Vector3<f32>,
) -> Point {
    let theta_x = rotation.x.clamp(0.0, 6.28);
    let theta_y = rotation.y.clamp(0.0, 6.28);
    let theta_z = rotation.z.clamp(0.0, 6.28);

    let rot_x = Rotation3::<f32>::from_euler_angles(theta_x, 0.0, 0.0);
    let rot_y = Rotation3::<f32>::from_euler_angles(0.0, theta_y, 0.0);
    let rot_z = Rotation3::<f32>::from_euler_angles(0.0, 0.0, theta_z);
    let diff = Vector3::<f32>::new(
        vec.x - cam_pos.x,
        vec.y - cam_pos.y,
        vec.z - cam_pos.z,
    );
    let v = rot_x * (rot_y * (rot_z * diff));

    let x = (((surf.z / v.z) * v.x) + surf.x).clamp(-120f32, 120f32);
    let y = (((surf.z / v.z) * v.y) + surf.x).clamp(-120f32, 120f32);

    Point {
        x: x as i32,
        y: y as i32,
    }
}

/// Takes an i16 number expected to be between 0 and 16384 (2^14) and scales
/// it to a radians value between 0.0 and 6.28.
fn scale_saadc_result(n: i16) -> f32 {
    let n = n.clamp(0, POT_PIN_MAX_READ);
    ((n as f32 / POT_PIN_MAX_READ as f32) * 6.28).clamp(0.0, 6.28)
}
