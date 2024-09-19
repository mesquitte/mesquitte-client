use mqtt_codec_kit::common::QualityOfService;

pub struct ClientOptions {
    server: String,
    client_id: String,
    username: String,
    password: String,
    clean_session: bool,
    will_enabled: bool,
    will_topic: String,
    will_payload: Vec<u8>,
    will_qos: QualityOfService,
    will_retained: bool,
    keep_alive: i32,
    connect_timeout: i32,
}

impl Default for ClientOptions {
    fn default() -> Self {
        Self {
            server: Default::default(),
            client_id: Default::default(),
            username: Default::default(),
            password: Default::default(),
            clean_session: Default::default(),
            will_enabled: Default::default(),
            will_topic: Default::default(),
            will_payload: Default::default(),
            will_qos: QualityOfService::Level0,
            will_retained: Default::default(),
            keep_alive: Default::default(),
            connect_timeout: Default::default(),
        }
    }
}

impl ClientOptions {
    pub fn new() -> Self {
        Self {
            keep_alive: 30,
            connect_timeout: 10,
            ..Default::default()
        }
    }

    pub fn set_server<S: Into<String>>(&mut self, addr: S) -> &mut Self {
        self.server = addr.into();
        self
    }

    pub fn set_client_id<S: Into<String>>(&mut self, client_id: S) -> &mut Self {
        self.client_id = client_id.into();
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

    pub fn set_clean_session(&mut self, clean: bool) -> &mut Self {
        self.clean_session = clean;
        self
    }

    pub fn set_will<S: Into<String>>(
        &mut self,
        topic: S,
        payload: Vec<u8>,
        qos: QualityOfService,
        retained: bool,
    ) -> &mut Self {
        self.will_enabled = true;
        self.will_topic = topic.into();
        self.will_payload = payload;
        self.will_qos = qos;
        self.will_retained = retained;
        self
    }

    pub fn unset_will(&mut self) -> &mut Self {
        self.will_enabled = false;
        self
    }

    pub fn server(&self) -> &String {
        &self.server
    }

    pub fn client_id(&self) -> &String {
        &self.client_id
    }

    pub fn username(&self) -> &String {
        &self.username
    }

    pub fn password(&self) -> &String {
        &self.password
    }

    pub fn clean_session(&self) -> bool {
        self.clean_session
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
}
