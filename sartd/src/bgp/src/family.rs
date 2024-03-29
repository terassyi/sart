use prost::Message;
use std::convert::TryFrom;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AddressFamily {
    pub afi: Afi,
    pub safi: Safi,
}

impl AddressFamily {
    pub fn new(afi: u16, safi: u8) -> Result<Self, &'static str> {
        let afi = Afi::try_from(afi)?;
        let safi = Safi::try_from(safi)?;
        Ok(Self { afi, safi })
    }

    pub fn ipv4_unicast() -> Self {
        Self {
            afi: Afi::IPv4,
            safi: Safi::Unicast,
        }
    }

    pub fn ipv6_unicast() -> Self {
        Self {
            afi: Afi::IPv6,
            safi: Safi::Unicast,
        }
    }
}

impl TryFrom<u32> for AddressFamily {
    type Error = &'static str;
    fn try_from(value: u32) -> Result<Self, Self::Error> {
        let afi = Afi::try_from((value >> 16) as u16)?;
        let safi = Safi::try_from(value as u8)?;
        Ok(Self { afi, safi })
    }
}

impl TryFrom<prost_types::Any> for AddressFamily {
    type Error = &'static str;
    fn try_from(value: prost_types::Any) -> Result<Self, Self::Error> {
        let a = match sartd_proto::sart::AddressFamily::decode(&*value.value) {
            Ok(a) => a,
            Err(_) => return Err("invalid argument"),
        };
        let afi = Afi::try_from(a.afi as u16)?;
        let safi = Safi::try_from(a.safi as u8)?;
        Ok(AddressFamily { afi, safi })
    }
}

impl From<&AddressFamily> for prost_types::Any {
    fn from(family: &AddressFamily) -> Self {
        sartd_util::to_any(
            sartd_proto::sart::AddressFamily {
                afi: family.afi as i32,
                safi: family.safi as i32,
            },
            "AddressFamily",
        )
    }
}

impl From<&AddressFamily> for sartd_proto::sart::AddressFamily {
    fn from(family: &AddressFamily) -> Self {
        sartd_proto::sart::AddressFamily {
            afi: family.afi as i32,
            safi: family.safi as i32,
        }
    }
}

impl From<AddressFamily> for u32 {
    fn from(val: AddressFamily) -> Self {
        (((val.afi as u16) as u32) << 16) + ((val.safi as u8) as u32)
    }
}

impl<'a> From<&'a AddressFamily> for u32 {
    fn from(val: &'a AddressFamily) -> Self {
        (((val.afi as u16) as u32) << 16) + ((val.safi as u8) as u32)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Afi {
    IPv4 = 1,
    IPv6 = 2,
}

impl TryFrom<u16> for Afi {
    type Error = &'static str;
    fn try_from(value: u16) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::IPv4),
            2 => Ok(Self::IPv6),
            _ => Err("invalid AFI"),
        }
    }
}

impl From<Afi> for u16 {
    fn from(val: Afi) -> Self {
        match val {
            Afi::IPv4 => 1,
            Afi::IPv6 => 2,
        }
    }
}

impl Afi {
    pub fn inet(&self) -> u8 {
        match self {
            Afi::IPv4 => 2,
            Afi::IPv6 => 10,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Safi {
    Unicast = 1,
    Multicast = 2,
}

impl TryFrom<u8> for Safi {
    type Error = &'static str;
    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::Unicast),
            2 => Ok(Self::Multicast),
            _ => Err("unsupported SAFI number"),
        }
    }
}

impl From<Safi> for u8 {
    fn from(val: Safi) -> Self {
        match val {
            Safi::Unicast => 1,
            Safi::Multicast => 2,
        }
    }
}

impl Safi {
    pub fn inet(&self) -> u8 {
        match self {
            Safi::Unicast => 1,
            Safi::Multicast => 5,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{AddressFamily, Afi, Safi};
    use rstest::rstest;

    #[rstest(input, expected,
        case(0x0001_0001, AddressFamily{afi: Afi::IPv4, safi: Safi::Unicast}),
        case(0x0001_0002, AddressFamily{afi: Afi::IPv4, safi: Safi::Multicast}),
        case(0x0002_0001, AddressFamily{afi: Afi::IPv6, safi: Safi::Unicast}),
        case(0x0002_0002, AddressFamily{afi: Afi::IPv6, safi: Safi::Multicast}),
    )]
    fn works_address_family_try_from(input: u32, expected: AddressFamily) {
        let family = AddressFamily::try_from(input).unwrap();
        assert_eq!(family, expected)
    }

    #[rstest(
        input,
        expected,
        case(0x0003_0001, "invalid AFI"),
        case(0x0001_0000, "unsupported SAFI number")
    )]
    fn failed_address_family_try_from(input: u32, expected: &'static str) {
        match AddressFamily::try_from(input) {
            Ok(_) => panic!("failed"),
            Err(e) => assert_eq!(e, expected),
        }
    }

    #[rstest(
        input,
        expected,
        case(&AddressFamily{afi: Afi::IPv4, safi: Safi::Unicast}, 0x00010001),
        case(&AddressFamily{afi: Afi::IPv6, safi: Safi::Unicast}, 0x00020001),
        case(&AddressFamily{afi: Afi::IPv6, safi: Safi::Multicast}, 0x00020002),
    )]
    fn works_address_family_into(input: &AddressFamily, expected: u32) {
        assert_eq!(expected, Into::<u32>::into(input));
    }
}
