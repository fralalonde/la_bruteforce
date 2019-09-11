use linked_hash_map::LinkedHashMap;
use std::iter::Iterator;

use crate::midi::{Result};
use std::str::FromStr;
use strum::IntoEnumIterator;
use midir::MidiOutput;
use crate::midi::MidiPort;

//mod beatstep;
mod microbrute;

pub type MidiValue = u8;

pub type DeviceId = &'static str;
pub type Parameter = &'static str;

#[derive(Debug, EnumString, IntoStaticStr, EnumIter)]
pub enum DeviceType {
    MicroBrute,
}

impl DeviceType {
    pub fn descriptor(&self) -> Box<Descriptor> {
        Box::new(match self {
            MicroBrute => microbrute::MicroBruteDescriptor {}
        })
    }
}


pub trait Descriptor {
    fn parameters(&self) -> Vec<Parameter>;
    fn bounds(&self, param: Parameter) -> Bounds;
    fn ports(&self) -> Vec<MidiPort>;
    fn connect(&self, port: &MidiPort) -> Result<Box<Device>>;
}

pub trait Device {
    fn query(&mut self, params: &[Parameter]) -> Result<Vec<(Parameter, MidiValue)>>;
    fn update(&mut self, param: Parameter, value: &str) -> Result<()>;
}

#[derive(Debug, Snafu)]
pub enum DeviceError {
    #[snafu(display("Invalid device name {}", device_name))]
    UnknownDevice {
        device_name: String,
    },
    UnknownParameter {
        param_name: String,
    },
    NoConnectedDevice {
        device_name: String,
    },
    NoOutputPort {
        port_name: String,
    },
    NoInputPort {
        port_name: String,
    },
    InvalidParam {
        device_name: String,
        param_name: String,
    },
    NoValueReceived,
    ValueOutOfBound {
        value_name: String,
    },
}


//#[derive(Debug)]
//enum HotplugEvent {
//    CONNECTED(&'static DeviceDescriptor),
//    DISCONNECTED(&'static DeviceDescriptor),
//}

//#[derive(Debug, Clone)]
//pub struct DeviceDescriptor<T: Parameter> {
//    fn parameters(&self) -> Vec<T>;
//    usb_vendor_id: u32,
//    usb_product_id: u32,
//    sysex_out_id: u8,
//    sysex_tx_id: u8,
//    port_name_prefix: &'static str,
//    name: &'static str,
//    params: Vec<Param>,
//}

//#[derive(Debug, Clone)]
//pub trait Param {
//    //    fn bounds(&self) -> ParameterBounds;
//    //    pub sysex_rx_id: SysexParamId,
//    //    pub sysex_tx_id: SysexParamId,
//    //    pub name: &'static str,
//    //    pub bounds: ParameterBounds,
//}

#[derive(Debug, Clone)]
pub enum Bounds {
    /// List of raw value and display name pairs
    Discrete(Vec<(MidiValue, &'static str)>),

    /// Raw value offset and display value bounds (Low to High, inclusive)
    Range(u8, (MidiValue, MidiValue)),
}
