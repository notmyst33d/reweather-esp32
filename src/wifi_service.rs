use embedded_svc::wifi::{self, Wifi};
use esp_idf_hal::modem::WifiModem;
use esp_idf_svc::{
    eventloop::{EspEventLoop, System},
    netif::{EspNetif, EspNetifWait},
    wifi::{EspWifi, WifiWait},
};
use std::{
    error::Error,
    net::Ipv4Addr,
    sync::{Arc, Mutex},
    time::Duration,
};

const TIMEOUT: u8 = 20;

pub struct WifiService<'a> {
    event_loop: EspEventLoop<System>,
    wifi: Arc<Mutex<EspWifi<'a>>>,
    ssid: &'a str,
    psk: &'a str,
    error_callback: fn(Box<dyn Error>),
}

impl<'a> WifiService<'static> {
    pub fn new(
        ssid: &'static str,
        psk: &'static str,
        error_callback: fn(Box<dyn Error>),
    ) -> Result<Self, Box<dyn Error>> {
        let event_loop = EspEventLoop::take()?;
        let wifi = Arc::new(Mutex::new(EspWifi::new(
            unsafe { WifiModem::new() },
            event_loop.clone(),
            None,
        )?));

        Ok(Self {
            event_loop,
            wifi,
            ssid,
            psk,
            error_callback,
        })
    }

    pub fn connect(
        event_loop: EspEventLoop<System>,
        wifi: Arc<Mutex<EspWifi<'a>>>,
        ssid: &'a str,
        psk: &'a str,
    ) -> Result<(), Box<dyn Error>> {
        let mut wifi = wifi.lock().unwrap();

        let scan = wifi.scan()?;
        let network = match scan.iter().find(|network| network.ssid == ssid) {
            Some(network) => network,
            None => return Err(format!("Cannot find {}", ssid).into()),
        };

        wifi.set_configuration(&wifi::Configuration::Client(wifi::ClientConfiguration {
            ssid: ssid.into(),
            password: psk.into(),
            auth_method: network.auth_method,
            bssid: Some(network.bssid),
            channel: Some(network.channel),
        }))?;

        if !wifi.is_started()? {
            wifi.start()?;
        }

        wifi.connect()?;

        if !WifiWait::new(&event_loop)?
            .wait_with_timeout(Duration::from_secs(TIMEOUT.into()), || {
                wifi.is_started().unwrap()
            })
        {
            return Err(format!(
                "Reached {TIMEOUT} second timeout while trying to connect to WiFi"
            )
            .into());
        }

        if !EspNetifWait::new::<EspNetif>(wifi.sta_netif(), &event_loop)?.wait_with_timeout(
            Duration::from_secs(TIMEOUT.into()),
            || {
                wifi.is_connected().unwrap()
                    && wifi.sta_netif().get_ip_info().unwrap().ip != Ipv4Addr::new(0, 0, 0, 0)
            },
        ) {
            return Err(format!(
                "Reached {TIMEOUT} second timeout while trying to connect to WiFi"
            )
            .into());
        }

        Ok(())
    }

    pub fn start(self) {
        std::thread::spawn(move || loop {
            let wifi_clone = self.wifi.clone();

            let is_connected = match wifi_clone.lock().unwrap().is_connected() {
                Ok(result) => result,
                Err(error) => {
                    (self.error_callback)(error.into());
                    return;
                }
            };

            if !is_connected {
                println!("WifiService: Reconnecting");
                match WifiService::connect(self.event_loop.clone(), wifi_clone, self.ssid, self.psk)
                {
                    Ok(_) => (),
                    Err(error) => println!("WifiService: WifiService::connect() failed: {error}"),
                };
            };

            std::thread::sleep(Duration::from_secs(1));
        });
    }
}
