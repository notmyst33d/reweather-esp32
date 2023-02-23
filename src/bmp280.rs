/*
    Mostly complete BMP280 driver
    Datasheet: https://www.bosch-sensortec.com/media/boschsensortec/downloads/datasheets/bst-bmp280-ds001.pdf
*/

#![allow(unused)]

use std::{error::Error, io};

use embedded_hal::i2c;

pub struct BMP280<I2C> {
    i2c: I2C,
    address: Address,
    calibration: Calibration,
}

impl<I2C, E> BMP280<I2C>
where
    I2C: i2c::I2c<Error = E>,
{
    pub fn new(i2c: I2C, address: Address) -> Result<Self, E> {
        let mut s = Self {
            i2c,
            address,
            calibration: Calibration::default(),
        };

        s.calibration.dig_t1 = s.read_register_u16(Register::DigT1)? as f64;
        s.calibration.dig_t2 = (s.read_register_u16(Register::DigT2)? as i16) as f64;
        s.calibration.dig_t3 = (s.read_register_u16(Register::DigT3)? as i16) as f64;

        s.calibration.dig_p1 = s.read_register_u16(Register::DigP1)? as f64;
        s.calibration.dig_p2 = (s.read_register_u16(Register::DigP2)? as i16) as f64;
        s.calibration.dig_p3 = (s.read_register_u16(Register::DigP3)? as i16) as f64;
        s.calibration.dig_p4 = (s.read_register_u16(Register::DigP4)? as i16) as f64;
        s.calibration.dig_p5 = (s.read_register_u16(Register::DigP5)? as i16) as f64;
        s.calibration.dig_p6 = (s.read_register_u16(Register::DigP6)? as i16) as f64;
        s.calibration.dig_p7 = (s.read_register_u16(Register::DigP7)? as i16) as f64;
        s.calibration.dig_p8 = (s.read_register_u16(Register::DigP8)? as i16) as f64;
        s.calibration.dig_p9 = (s.read_register_u16(Register::DigP9)? as i16) as f64;

        Ok(s)
    }

    pub fn id(&mut self) -> Result<u8, E> {
        self.read_register_u8(Register::ID)
    }

    pub fn status(&mut self) -> Result<u8, E> {
        self.read_register_u8(Register::Status)
    }

    pub fn control(&mut self) -> Result<u8, E> {
        self.read_register_u8(Register::Control)
    }

    pub fn set_control(&mut self, value: u8) -> Result<(), E> {
        self.write_register_u8(Register::Control, value)
    }

    pub fn config(&mut self) -> Result<u8, E> {
        self.read_register_u8(Register::Config)
    }

    pub fn set_config(&mut self, value: u8) -> Result<(), E> {
        self.write_register_u8(Register::Config, value)
    }

    /*
        I dont like this nested Result
        But thats the best thing i could do
        to return an error while easily
        propagating I2C implementation errors
    */
    pub fn pressure(&mut self) -> Result<Result<f64, Box<dyn Error>>, E> {
        let mut raw = self.read_register_bytes(Register::Temperature, 3)?;
        let adc_t =
            (((raw[0] as u32) << 12 | (raw[1] as u32) << 4 | (raw[2] as u32) >> 4) as i32) as f64;

        raw = self.read_register_bytes(Register::Pressure, 3)?;
        let adc_p =
            (((raw[0] as u32) << 12 | (raw[1] as u32) << 4 | (raw[2] as u32) >> 4) as i32) as f64;

        let mut var1 =
            (adc_t / 16384.0 - self.calibration.dig_t1 / 1024.0) * (self.calibration.dig_t2);
        let mut var2 = ((adc_t / 131072.0 - self.calibration.dig_t1 / 8192.0)
            * (adc_t / 131072.0 - self.calibration.dig_t1 / 8192.0))
            * self.calibration.dig_t3;

        var1 = (var1 + var2) / 2.0 - 64000.0;
        var2 = var1 * var1 * self.calibration.dig_p6 / 32768.0;
        var2 = var2 + var1 * self.calibration.dig_p5 * 2.0;
        var2 = (var2 / 4.0) + (self.calibration.dig_p4 * 65536.0);
        var1 = (self.calibration.dig_p3 * var1 * var1 / 524288.0 + self.calibration.dig_p2 * var1)
            / 524288.0;
        var1 = (1.0 + var1 / 32768.0) * self.calibration.dig_p1;

        let mut pressure = 1048576.0 - adc_p;
        if var1 < 0.0 || var1 > 0.0 {
            pressure = (pressure - (var2 / 4096.0)) * 6250.0 / var1;
            var1 = self.calibration.dig_p9 * pressure * pressure / 2147483648.0;
            var2 = pressure * self.calibration.dig_p8 / 32768.0;
            pressure = pressure + (var1 + var2 + self.calibration.dig_p7) / 16.0;
        } else {
            return Ok(Err("Pressure compensation failed".into()));
        }

        Ok(Ok(pressure))
    }

    pub fn temperature(&mut self) -> Result<f64, E> {
        let raw = self.read_register_bytes(Register::Temperature, 3)?;
        let adc_t =
            (((raw[0] as u32) << 12 | (raw[1] as u32) << 4 | (raw[2] as u32) >> 4) as i32) as f64;

        let var1 = (adc_t / 16384.0 - self.calibration.dig_t1 / 1024.0) * (self.calibration.dig_t2);
        let var2 = ((adc_t / 131072.0 - self.calibration.dig_t1 / 8192.0)
            * (adc_t / 131072.0 - self.calibration.dig_t1 / 8192.0))
            * self.calibration.dig_t3;
        let temperature = (var1 + var2) / 5120.0;

        Ok(temperature)
    }

    fn write_register_u8(&mut self, register: Register, value: u8) -> Result<(), E> {
        self.i2c.write(self.address as u8, &[register as u8, value])
    }

    fn read_register_u8(&mut self, register: Register) -> Result<u8, E> {
        let mut data: [u8; 1] = [0; 1];
        self.i2c
            .write_read(self.address as u8, &[register as u8], &mut data)?;
        Ok(u8::from_le_bytes(data))
    }

    fn read_register_u16(&mut self, register: Register) -> Result<u16, E> {
        let mut data: [u8; 2] = [0; 2];
        self.i2c
            .write_read(self.address as u8, &[register as u8], &mut data)?;
        Ok(u16::from_le_bytes(data))
    }

    fn read_register_bytes(&mut self, register: Register, length: usize) -> Result<Vec<u8>, E> {
        let mut data = vec![0; length];
        self.i2c
            .write_read(self.address as u8, &[register as u8], &mut data)?;
        Ok(data)
    }
}

#[derive(Clone, Copy)]
pub enum Address {
    Primary = 0x76,
    Secondary = 0x77,
}

pub enum Register {
    ID = 0xD0,
    DigT1 = 0x88,
    DigT2 = 0x8A,
    DigT3 = 0x8C,
    DigP1 = 0x8E,
    DigP2 = 0x90,
    DigP3 = 0x92,
    DigP4 = 0x94,
    DigP5 = 0x96,
    DigP6 = 0x98,
    DigP7 = 0x9A,
    DigP8 = 0x9C,
    DigP9 = 0x9E,
    Status = 0xF3,
    Control = 0xF4,
    Config = 0xF5,
    Pressure = 0xF7,
    Temperature = 0xFA,
}

// TODO: Sleep and forced mode
pub enum Control {
    TemperatureOversamplingX1 = 0b001_000_00,
    TemperatureOversamplingX2 = 0b010_000_00,
    TemperatureOversamplingX4 = 0b011_000_00,
    TemperatureOversamplingX8 = 0b100_000_00,
    TemperatureOversamplingX16 = 0b101_000_00,
    PressureOversamplingX1 = 0b000_001_00,
    PressureOversamplingX2 = 0b000_010_00,
    PressureOversamplingX4 = 0b000_011_00,
    PressureOversamplingX8 = 0b000_100_00,
    PressureOversamplingX16 = 0b000_101_00,
    NormalMode = 0b000_000_11,
}

// TODO: filter[2:0]
pub enum Config {
    Standby62_5MS = 0b001_000_00,
    Standby125MS = 0b010_000_00,
    Standby250MS = 0b011_000_00,
    Standby500MS = 0b100_000_00,
    Standby1000MS = 0b101_000_00,
    Standby2000MS = 0b110_000_00,
    Standby4000MS = 0b111_000_00,
}

#[derive(Default)]
struct Calibration {
    dig_t1: f64,
    dig_t2: f64,
    dig_t3: f64,
    dig_p1: f64,
    dig_p2: f64,
    dig_p3: f64,
    dig_p4: f64,
    dig_p5: f64,
    dig_p6: f64,
    dig_p7: f64,
    dig_p8: f64,
    dig_p9: f64,
}
