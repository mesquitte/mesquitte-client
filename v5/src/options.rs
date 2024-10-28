use std::time::Duration;

use mqtt_codec_kit::{common::QualityOfService, v5::packet::connect::LastWillProperties};

pub struct ClientOptions {
    server: String,
    client_id: Option<String>,
    username: String,
    password: String,
    clean_start: bool,
    will_enabled: bool,
    will_topic: String,
    will_payload: Vec<u8>,
    will_qos: QualityOfService,
    will_retained: bool,
    will_properties: LastWillProperties,
    keep_alive: Duration,
    connect_timeout: Duration,
    auto_reconnect: bool,
    connect_retry_interval: Duration,
    max_connect_retry_times: Option<u16>,
}

impl Default for ClientOptions {
    fn default() -> Self {
        Self {
            server: Default::default(),
            client_id: Default::default(),
            username: Default::default(),
            password: Default::default(),
            clean_start: Default::default(),
            will_enabled: Default::default(),
            will_topic: Default::default(),
            will_payload: Default::default(),
            will_qos: QualityOfService::Level0,
            will_retained: Default::default(),
            will_properties: Default::default(),
            keep_alive: Default::default(),
            connect_timeout: Default::default(),
            auto_reconnect: Default::default(),
            connect_retry_interval: Default::default(),
            max_connect_retry_times: Default::default(),
        }
    }
}

impl ClientOptions {
    pub fn new() -> Self {
        Self {
            keep_alive: Duration::from_secs(30),
            connect_timeout: Duration::from_secs(10),
            connect_retry_interval: Duration::from_secs(5),
            ..Default::default()
        }
    }

    pub fn set_server<S: Into<String>>(&mut self, addr: S) -> &mut Self {
        self.server = addr.into();
        self
    }

    pub fn set_client_id<S: Into<String>>(&mut self, client_id: S) -> &mut Self {
        self.client_id = Some(client_id.into());
        self
    }

    pub fn set_username<S: Into<String>>(&mut self, username: S) -> &mut Self {
        self.username = username.into();
        self
    }

    pub fn set_password<S: Into<String>>(&mut self, password: S) -> &mut Self {
        self.password = password.into();
        self
    }

    pub fn set_clean_start(&mut self, clean: bool) -> &mut Self {
        self.clean_start = clean;
        self
    }

    pub fn set_will<S: Into<String>>(
        &mut self,
        topic: S,
        payload: Vec<u8>,
        qos: QualityOfService,
        retained: bool,
        properties: LastWillProperties,
    ) -> &mut Self {
        self.will_enabled = true;
        self.will_topic = topic.into();
        self.will_payload = payload;
        self.will_qos = qos;
        self.will_retained = retained;
        self.will_properties = properties;
        self
    }

    pub fn unset_will(&mut self) -> &mut Self {
        self.will_enabled = false;
        self
    }

    pub fn set_keep_alive(&mut self, keep_alive: Duration) -> &mut Self {
        self.keep_alive = keep_alive;
        self
    }

    pub fn set_connect_timeout(&mut self, connect_timeout: Duration) -> &mut Self {
        self.connect_timeout = connect_timeout;
        self
    }

    pub fn set_auto_reconnect(&mut self, flag: bool) -> &mut Self {
        self.auto_reconnect = flag;
        self
    }

    pub fn set_connect_retry_interval(&mut self, interval: Duration) -> &mut Self {
        self.connect_retry_interval = interval;
        self
    }

    pub fn set_max_connect_retry_times(&mut self, times: Option<u16>) -> &mut Self {
        self.max_connect_retry_times = times;
        self
    }

    pub fn server(&self) -> &String {
        &self.server
    }

    pub fn client_id(&self) -> &Option<String> {
        &self.client_id
    }

    pub fn username(&self) -> &String {
        &self.username
    }

    pub fn password(&self) -> &String {
        &self.password
    }

    pub fn clean_start(&self) -> bool {
        self.clean_start
    }

    pub fn will_enabled(&self) -> bool {
        self.will_enabled
    }

    pub fn will_topic(&self) -> &String {
        &self.will_topic
    }

    pub fn will_payload(&self) -> &Vec<u8> {
        &self.will_payload
    }

    pub fn will_qos(&self) -> QualityOfService {
        self.will_qos
    }

    pub fn will_retained(&self) -> bool {
        self.will_retained
    }

    pub fn will_properties(&self) -> LastWillProperties {
        self.will_properties.clone()
    }

    pub fn keep_alive(&self) -> Duration {
        self.keep_alive
    }

    pub fn connect_timeout(&self) -> Duration {
        self.connect_timeout
    }

    pub fn auto_reconnect(&self) -> bool {
        self.auto_reconnect
    }

    pub fn connect_retry_interval(&self) -> Duration {
        self.connect_retry_interval
    }

    pub fn max_connect_retry_times(&self) -> Option<u16> {
        self.max_connect_retry_times
    }
}
