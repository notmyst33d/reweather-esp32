use std::{
    error::Error,
    io::{self, Write},
    net::{Shutdown, TcpListener},
    sync::{Arc, Mutex},
};

use esp_idf_hal::i2c::I2cDriver;

use crate::bmp280;

pub struct NetworkService<'a> {
    driver: Arc<Mutex<bmp280::BMP280<I2cDriver<'a>>>>,
    socket: TcpListener,
    error_callback: fn(Box<dyn Error>),
}

impl<'a> NetworkService<'static> {
    pub fn new(
        port: u16,
        driver: Arc<Mutex<bmp280::BMP280<I2cDriver<'static>>>>,
        error_callback: fn(Box<dyn Error>),
    ) -> Result<Self, io::Error> {
        Ok(Self {
            driver,
            socket: TcpListener::bind(format!("0.0.0.0:{}", port))?,
            error_callback,
        })
    }

    pub fn start(self) {
        let driver_clone = self.driver.clone();

        std::thread::spawn(move || loop {
            for client in self.socket.incoming() {
                let mut client = match client {
                    Ok(result) => result,
                    Err(error) => {
                        (self.error_callback)(error.into());
                        return;
                    }
                };

                let temperature = match driver_clone.lock().unwrap().temperature() {
                    Ok(result) => result,
                    Err(error) => {
                        (self.error_callback)(error.into());
                        return;
                    }
                };

                let pressure = match driver_clone.lock().unwrap().pressure() {
                    Ok(result) => match result {
                        Ok(result) => result,
                        Err(error) => {
                            (self.error_callback)(error.into());
                            return;
                        }
                    },
                    Err(error) => {
                        (self.error_callback)(error.into());
                        return;
                    }
                };

                match client.write_all(
                    [temperature.to_le_bytes(), pressure.to_le_bytes()]
                        .concat()
                        .as_slice(),
                ) {
                    Ok(_) => (),
                    Err(error) => {
                        (self.error_callback)(error.into());
                        return;
                    }
                };

                match client.shutdown(Shutdown::Both) {
                    Ok(_) => (),
                    Err(error) => {
                        (self.error_callback)(error.into());
                        return;
                    }
                };
            }
        });
    }
}
