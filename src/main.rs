mod bmp280;
mod network_service;
mod wifi_service;

use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use esp_idf_hal::{
    i2c::{self},
    prelude::Peripherals,
};

use crate::{network_service::NetworkService, wifi_service::WifiService};

const CONFIG: &str = include_str!("../config.txt");

fn main() -> std::io::Result<()> {
    esp_idf_sys::link_patches();

    println!("Reweather ESP32");

    let mut ssid: Option<&str> = None;
    let mut psk: Option<&str> = None;
    let mut port: Option<u16> = None;

    for line in CONFIG.lines().into_iter() {
        let data: Vec<&str> = line.split("=").collect();
        match data[0] {
            "ssid" => ssid = Some(data[1]),
            "psk" => psk = Some(data[1]),
            "port" => {
                port = Some(match data[1].parse() {
                    Ok(result) => result,
                    Err(error) => panic!("PANIC! Cannot parse \"{}\": {error}", data[0]),
                })
            }
            &_ => panic!("PANIC! Unknown field \"{}\"", data[0]),
        };
    }

    if ssid.is_none() || psk.is_none() || port.is_none() {
        panic!("PANIC! config.txt doesnt have \"ssid\", \"psk\" or \"port\" fields");
    }

    println!("Initializing Peripherals");
    let peripherals = match Peripherals::take() {
        Some(peripherals) => peripherals,
        None => panic!("PANIC! Cannot create Peripherals instance"),
    };

    println!("Initializing I2cDriver");
    let driver = match i2c::I2cDriver::new(
        peripherals.i2c0,
        peripherals.pins.gpio32,
        peripherals.pins.gpio33,
        &i2c::config::Config::default(),
    ) {
        Ok(result) => result,
        Err(error) => panic!("PANIC! Cannot create I2cDriver instance: {error}"),
    };

    println!("Initializing BMP280 driver");
    let sensor = match bmp280::BMP280::new(driver, bmp280::Address::Primary) {
        Ok(result) => Arc::new(Mutex::new(result)),
        Err(error) => panic!("PANIC! Cannot create BMP280 driver instance: {error}"),
    };

    println!("Initializing BMP280");
    match sensor
        .lock()
        .unwrap()
        .set_config(bmp280::Config::Standby4000MS as u8)
    {
        Ok(_) => (),
        Err(error) => panic!("PANIC! sensor.set_config() failed: {error}"),
    };

    match sensor.lock().unwrap().set_control(
        bmp280::Control::TemperatureOversamplingX1 as u8
            | bmp280::Control::PressureOversamplingX1 as u8
            | bmp280::Control::NormalMode as u8,
    ) {
        Ok(_) => (),
        Err(error) => panic!("PANIC! sensor.set_control() failed: {error}"),
    };

    println!("BMP280 initialized:");
    println!("  ID: {}", sensor.lock().unwrap().id().unwrap());
    println!("  Config: {}", sensor.lock().unwrap().config().unwrap());
    println!("  Control: {}", sensor.lock().unwrap().control().unwrap());

    println!("Initializing services");

    println!("Starting WifiService");
    let wifi_service = match WifiService::new(ssid.unwrap(), psk.unwrap(), |error| {
        panic!("PANIC! WifiService raised an error. ({error})")
    }) {
        Ok(result) => result,
        Err(error) => panic!("PANIC! Cannot create WifiService instance: {error}"),
    };
    wifi_service.start();

    println!("Starting NetworkService");
    let network_service = match NetworkService::new(port.unwrap(), sensor.clone(), |error| {
        panic!("PANIC! NetworkService raised an error. ({error})")
    }) {
        Ok(result) => result,
        Err(error) => panic!("PANIC! Cannot create NetworkService instance: {error}"),
    };
    network_service.start();

    println!("System ready");
    loop {
        println!(
            "Temperature: {} °C",
            sensor.lock().unwrap().temperature().unwrap()
        );
        println!(
            "Pressure: {} Pa",
            sensor.lock().unwrap().pressure().unwrap().unwrap()
        );
        std::thread::sleep(Duration::from_secs(1));
    }
}
