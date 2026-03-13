#![no_main]
#![no_std]

use cortex_m_rt::entry;
use embedded_graphics::{
    Drawable,
    pixelcolor::Rgb565,
    prelude::*,
    primitives::{Line, PrimitiveStyle},
};
use embedded_hal::delay::DelayNs;
use embedded_hal_bus::spi::ExclusiveDevice;
use microbit::hal::{
    Spim,
    gpio::Level,
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
use rtt_target::rprintln;
use rtt_target::rtt_init_print;

const FRAME_TIME_MS: u32 = 100;

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

    // Vertices for a tetrahedron.
    let mut vertices3d: [Point3D; 4] = [
        Point3D::new(10.0f32, 10.0f32, 10.0f32),
        Point3D::new(10.0f32, -10.0f32, -10.0f32),
        Point3D::new(-10.0f32, 10.0f32, -10.0f32),
        Point3D::new(-10.0f32, -10.0f32, 10.0f32),
    ];

    // Edges on the tetrahedron corresponding to points in the array.
    let edges = [(0, 1), (0, 2), (0, 3), (1, 2), (1, 3), (2, 3)];

    // The 2D points to draw edges between.
    let mut points: [Point; 4] = [
        Point::new(0, 0),
        Point::new(0, 120),
        Point::new(120, 0),
        Point::new(-120, 0),
    ];

    for v in vertices3d.iter_mut() {
        *v = rotate_vertex(&v, 0.5, 0.0, 0.0);
    }

    for (i, v) in vertices3d.iter().enumerate() {
        points[i] = convert_3d_to_2d_point(v);
    }

    convert_points_to_display_coords(&mut points);

    rprintln!("Point {:?}", points[0]);
    rprintln!("Point {:?}", points[1]);
    rprintln!("Point {:?}", points[2]);
    rprintln!("Point {:?}", points[3]);

    for e in edges {
        Line::new(
            Point::new(points[e.0].x, points[e.0].y),
            Point::new(points[e.1].x, points[e.1].y),
        )
        .into_styled(PrimitiveStyle::with_stroke(Rgb565::BLACK, 5))
        .draw(&mut display)
        .unwrap();
    }

    loop {
        timer0.delay_ms(FRAME_TIME_MS);
    }
}

fn convert_3d_to_2d_point(p: &Point3D) -> Point {
    let x = (p.x / p.z) * 25f32 + 0f32;
    let y = (p.y / p.z) * 25f32 + 0f32;
    Point {
        x: x as i32,
        y: y as i32,
    }
}

/// Converts to display coords which range from 0 to 240 on each axis.
fn convert_points_to_display_coords(points: &mut [Point]) {
    for p in points {
        *p = Point::new(p.x + 120, p.y + 120);
    }
}

fn rotate_vertex(point: &Point3D, pitch: f32, yaw: f32, roll: f32) -> Point3D {
    let rot_x = Rotation3::<f32>::from_euler_angles(pitch, 0.0, 0.0);
    let rot_y = Rotation3::<f32>::from_euler_angles(0.0, yaw, 0.0);
    let rot_z = Rotation3::<f32>::from_euler_angles(0.0, 0.0, roll);
    let rotation = rot_z * rot_y * rot_x;
    let mut v = Vector3::new(point.x, point.y, point.z);
    v = rotation * v;
    Point3D::new(v.x, v.y, v.z)
}

struct Point3D {
    x: f32,
    y: f32,
    z: f32,
}

impl Point3D {
    fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }
}
