#![no_main]
#![no_std]

use cortex_m_rt::entry;
use embedded_graphics::{
    Drawable,
    pixelcolor::Rgb565,
    prelude::*,
    primitives::{Line, PrimitiveStyle},
};
// use embedded_hal::delay::DelayNs;
use embedded_hal_bus::spi::ExclusiveDevice;
use lsm303agr::{AccelMode, AccelOutputDataRate, Lsm303agr};
use microbit::hal::{
    Spim,
    gpio::Level,
    pac::twim0::frequency::FREQUENCY_A,
    spim::{self, Frequency},
    timer::Timer,
    twim::Twim,
};
use mipidsi::{
    Builder,
    models::GC9A01,
    options::{ColorInversion, Orientation, Rotation},
};
use nalgebra::{Rotation3, Vector3};
use panic_rtt_target as _;
use rtt_target::rtt_init_print;

const EDGE_COUNT: usize = 8;
const VERT_COUNT: usize = 5;

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

    // Set up lsm303agr
    let i2c =
        Twim::new(board.TWIM0, board.i2c_internal.into(), FREQUENCY_A::K100);
    let mut sensor = Lsm303agr::new_with_i2c(i2c);
    sensor.init().unwrap();
    sensor
        .set_accel_mode_and_odr(
            &mut timer0,
            AccelMode::HighResolution,
            AccelOutputDataRate::Hz50,
        )
        .unwrap();

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

    loop {
        let (x, y, z) = sensor.acceleration().unwrap().xyz_mg();
        let (rot_x, rot_y, rot_z) = convert_accel_to_rotation(x, y, z);
        let rot_mat = calculate_rotation_matrix(rot_x, rot_y, rot_z);

        // Vertices for a tetrahedron.
        let mut vertices3d: [Vector3<f32>; VERT_COUNT] = [
            Vector3::new(0.0f32, 0.0f32, 10.0f32),
            Vector3::new(-10.0f32, 10.0f32, -10.0f32),
            Vector3::new(10.0f32, 10.0f32, -10.0f32),
            Vector3::new(10.0f32, -10.0f32, -10.0f32),
            Vector3::new(-10.0f32, -10.0f32, -10.0f32),
        ];

        for v in vertices3d.iter_mut() {
            *v = transform_vertex(v, &rot_mat);
        }

        for (i, v) in vertices3d.iter().enumerate() {
            points[i] = convert_3d_to_2d_point(v);
        }

        convert_points_to_display_coords(&mut points);

        display.clear(Rgb565::BLACK).unwrap();

        for (i, edge) in edges.iter().enumerate() {
            Line::new(
                Point::new(points[edge.0].x, points[edge.0].y),
                Point::new(points[edge.1].x, points[edge.1].y),
            )
            .into_styled(PrimitiveStyle::with_stroke(edge_colors[i], 5))
            .draw(&mut display)
            .unwrap();
        }

        // timer0.delay_ms(FRAME_TIME_MS);
    }
}

/// Projects a 3D vertex to a 2D point.
fn convert_3d_to_2d_point(v: &Vector3<f32>) -> Point {
    let x = (v.x / v.z) * 20f32 + 0f32;
    let y = (v.y / v.z) * 20f32 + 0f32;
    Point {
        x: x as i32,
        y: y as i32,
    }
}

/// Converts to display coords which range from 0 to 240 on each axis.
fn convert_points_to_display_coords(points: &mut [Point]) {
    for p in points {
        *p = Point::new(p.x + 119, p.y + 119);
    }
}

/// Rotates a vertex based on the given angles.
fn transform_vertex(
    vec: &Vector3<f32>,
    rot_mat: &Rotation3<f32>,
) -> Vector3<f32> {
    rot_mat * vec
}

/// Rotates a vertex based on the given angles.
fn calculate_rotation_matrix(
    pitch: f32,
    yaw: f32,
    roll: f32,
) -> Rotation3<f32> {
    let rot_x = Rotation3::<f32>::from_euler_angles(pitch, 0.0, 0.0);
    let rot_y = Rotation3::<f32>::from_euler_angles(0.0, yaw, 0.0);
    let rot_z = Rotation3::<f32>::from_euler_angles(0.0, 0.0, roll);
    rot_z * rot_y * rot_x
}

/// Converts the accel valyues from the lsm303agr to rotation angles.
fn convert_accel_to_rotation(x: i32, y: i32, z: i32) -> (f32, f32, f32) {
    let (x, y, z) = convert_axes(x, y, z);
    (x as f32 / 500.0, y as f32 / 500.0, z as f32 / 500.0)
}

/// Converts the 3 axes from the lsm303agr crate acceleration() and xyz_mg()
/// functions and flips the axes to match the microbit board, such that the
/// the top is the where the USB port is.
fn convert_axes(x: i32, y: i32, z: i32) -> (i32, i32, i32) {
    (-x, -z, y)
}
