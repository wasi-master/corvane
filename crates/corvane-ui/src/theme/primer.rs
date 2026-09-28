//! Primer colour scale as vendored by GitHub Desktop (`primer-support` v4 era).
//! Sass `darken()`/`lighten()` results are precomputed (HSL lightness ± points).

#![allow(dead_code)]

pub const WHITE: u32 = 0xffffff;
pub const BLACK: u32 = 0x1b1f23;

pub const GRAY_000: u32 = 0xfafbfc;
pub const GRAY_100: u32 = 0xf6f8fa;
pub const GRAY_200: u32 = 0xe1e4e8;
pub const GRAY_300: u32 = 0xd1d5da;
pub const GRAY_400: u32 = 0x959da5;
pub const GRAY_500: u32 = 0x6a737d;
pub const GRAY_600: u32 = 0x586069;
pub const GRAY_700: u32 = 0x444d56;
pub const GRAY_800: u32 = 0x2f363d;
pub const GRAY_900: u32 = 0x24292e;

pub const BLUE_000: u32 = 0xf1f8ff;
pub const BLUE_100: u32 = 0xdbedff;
pub const BLUE_200: u32 = 0xc8e1ff;
pub const BLUE_300: u32 = 0x79b8ff;
pub const BLUE_400: u32 = 0x2188ff;
pub const BLUE_500: u32 = 0x0366d6;
pub const BLUE_600: u32 = 0x005cc5;
pub const BLUE_700: u32 = 0x044289;
pub const BLUE_800: u32 = 0x032f62;
pub const BLUE_900: u32 = 0x05264c;

pub const GREEN_000: u32 = 0xf0fff4;
pub const GREEN_100: u32 = 0xdcffe4;
pub const GREEN_200: u32 = 0xbef5cb;
pub const GREEN_300: u32 = 0x85e89d;
pub const GREEN_400: u32 = 0x34d058;
pub const GREEN_500: u32 = 0x28a745;
pub const GREEN_600: u32 = 0x22863a;
pub const GREEN_700: u32 = 0x176f2c;
pub const GREEN_800: u32 = 0x165c26;
pub const GREEN_900: u32 = 0x144620;

pub const RED_000: u32 = 0xffeef0;
pub const RED_100: u32 = 0xffdce0;
pub const RED_200: u32 = 0xfdaeb7;
pub const RED_300: u32 = 0xf97583;
pub const RED_400: u32 = 0xea4a5a;
pub const RED_500: u32 = 0xd73a49;
pub const RED_600: u32 = 0xcb2431;
pub const RED_700: u32 = 0xb31d28;
pub const RED_800: u32 = 0x9e1c23;
pub const RED_900: u32 = 0x86181d;

pub const YELLOW_000: u32 = 0xfffdef;
pub const YELLOW_100: u32 = 0xfffbdd;
pub const YELLOW_200: u32 = 0xfff5b1;
pub const YELLOW_300: u32 = 0xffea7f;
pub const YELLOW_400: u32 = 0xffdf5d;
pub const YELLOW_500: u32 = 0xffd33d;
pub const YELLOW_600: u32 = 0xf9c513;
pub const YELLOW_700: u32 = 0xdbab09;
pub const YELLOW_800: u32 = 0xb08800;
pub const YELLOW_900: u32 = 0x735c0f;

pub const ORANGE_000: u32 = 0xfff8f2;
pub const ORANGE_100: u32 = 0xffebda;
pub const ORANGE_200: u32 = 0xffd1ac;
pub const ORANGE_300: u32 = 0xffab70;
pub const ORANGE_400: u32 = 0xfb8532;
pub const ORANGE_500: u32 = 0xf66a0a;
pub const ORANGE_600: u32 = 0xe36209;
pub const ORANGE_700: u32 = 0xd15704;
pub const ORANGE_800: u32 = 0xc24e00;
pub const ORANGE_900: u32 = 0xa04100;

pub const PURPLE_300: u32 = 0xb392f0;

pub const BLUE: u32 = BLUE_500;
pub const GREEN: u32 = GREEN_500;
pub const RED: u32 = RED_500;
pub const YELLOW: u32 = YELLOW_500;
pub const ORANGE: u32 = ORANGE_500;
pub const LINK: u32 = BLUE;

// Precomputed Sass adjustments (see .docs/ghd-theme-tokens.md).
pub const GRAY_900_DARKEN_1: u32 = 0x22262b;
pub const GRAY_900_DARKEN_3: u32 = 0x1d2125;
pub const GRAY_900_DARKEN_6: u32 = 0x171a1d;
pub const GRAY_900_LIGHTEN_3: u32 = 0x2b3137;
pub const GRAY_700_LIGHTEN_5: u32 = 0x4f5a64;
pub const GRAY_500_LIGHTEN_3: u32 = 0x717b85;
pub const GRAY_500_LIGHTEN_30: u32 = 0xbbc0c5;
pub const GRAY_500_DARKEN_10: u32 = 0x535a61;
pub const GRAY_400_DARKEN_5: u32 = 0x879099;
pub const GRAY_100_DARKEN_20: u32 = 0xb4c5d6;
pub const BLUE_LIGHTEN_3: u32 = 0x036de5;
pub const BLUE_LIGHTEN_5: u32 = 0x0372ef;
pub const BLUE_200_DARKEN_5: u32 = 0xaed3ff;
pub const YELLOW_700_DARKEN_10: u32 = 0xaa8507;
pub const GREEN_000_DARKEN_2: u32 = 0xe6ffed;
pub const GREEN_100_DARKEN_3: u32 = 0xcdffd8;
pub const GREEN_900_DARKEN_2: u32 = 0x123e1c;
pub const GREEN_900_DARKEN_3: u32 = 0x113a1b;
pub const GREEN_900_DARKEN_8: u32 = 0x0b2611;
pub const RED_900_DARKEN_3: u32 = 0x79161a;
pub const RED_900_DARKEN_10: u32 = 0x5b1014;
pub const RED_900_DARKEN_15: u32 = 0x450c0f;
pub const RED_900_DARKEN_20: u32 = 0x2f080a;
