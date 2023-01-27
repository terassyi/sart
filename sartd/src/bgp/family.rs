use std::convert::TryFrom;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct AddressFamily {
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

impl Into<u32> for AddressFamily {
    fn into(self) -> u32 {
        (((self.afi as u16) as u32) << 16) + ((self.safi as u8) as u32)
    }
}

impl<'a> Into<u32> for &'a AddressFamily {
    fn into(self) -> u32 {
        (((self.afi as u16) as u32) << 16) + ((self.safi as u8) as u32)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Afi {
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

impl Into<u16> for Afi {
    fn into(self) -> u16 {
        match self {
            Self::IPv4 => 1,
            Self::IPv6 => 2,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Safi {
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

impl Into<u8> for Safi {
    fn into(self) -> u8 {
        match self {
            Self::Unicast => 1,
            Self::Multicast => 2,
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
            Ok(_) => assert!(false),
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
        assert_eq!(expected, input.into());
    }
}
