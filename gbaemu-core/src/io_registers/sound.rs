use super::*;
use crate::bitmod::*;
use crate::memory::MemoryPlugin;

#[derive(Debug, Default)]
pub struct WaveRam {
    cursor: usize,
    buffer: [u8; 0x20],
}
impl WaveRam {
    fn read_byte(&self, address: usize) -> u8 {
        self.buffer[self.real_address(address)]
    }

    fn write_byte(&mut self, address: usize, byte: u8) {
        self.buffer[self.real_address(address)] = byte;
    }

    fn real_address(&self, address: usize) -> usize {
        (self.cursor + address) % self.buffer.len()
    }
}

#[derive(Debug, Default)]
pub struct Sound {
    channel_1_sweep_register: u16,
    channel_1_duty_length_envelope: u16,
    channel_1_frequency_control: u16,
    channel_2_frequency_control: u16,
    channel_2_duty_length_envelope: u16,
    control_stereo_volumne_enable: u16,
    control_mixing_dma: u16,
    control_sound_on_off: u16,
    sound_pwm_control: u16,
    channel_3_stop_wave_ram_select: u16,
    channel_3_length_volume: u16,
    channel_3_frequency_control: u16,
    channel_3_wave_pattern: [WaveRam; 2],
    channel_4_length_envelope: u16,
    channel_4_frequency_control: u16,
    channel_a_fifo: u32,
    channel_b_fifo: u32,
}
impl Sound {
    fn inactive_channel_3_wave_pattern(&self) -> &WaveRam {
        let index = !self.channel_3_stop_wave_ram_select.bit(6) as usize;
        &self.channel_3_wave_pattern[index]
    }

    fn inactive_channel_3_wave_pattern_mut(&mut self) -> &mut WaveRam {
        let index = !self.channel_3_stop_wave_ram_select.bit(6) as usize;
        &mut self.channel_3_wave_pattern[index]
    }
}

impl MemoryPlugin for Sound {
    fn claims_address(&self, address: usize) -> bool {
        (0x4000060..0x40000B0).contains(&address)
    }

    fn read_byte(&self, address: usize) -> u8 {
        let relative_address = address & !0x4000000;
        match relative_address {
            0x60..0x62 => {
                read_byte_from_half_word(self.channel_1_sweep_register, relative_address - 0x60)
            }
            0x62..0x64 => read_byte_from_half_word(
                self.channel_1_duty_length_envelope,
                relative_address - 0x62,
            ),
            0x64..0x66 => {
                read_byte_from_half_word(self.channel_1_frequency_control, relative_address - 0x64)
            }
            0x66..0x68 => {
                todo!()
            }
            0x68..0x6a => read_byte_from_half_word(
                self.channel_2_duty_length_envelope,
                relative_address - 0x68,
            ),
            0x6a..0x6c => {
                todo!()
            }
            0x6C..0x6E => {
                read_byte_from_half_word(self.channel_2_frequency_control, relative_address - 0x6C)
            }
            0x6e..0x70 => {
                todo!()
            }
            0x70..0x72 => read_byte_from_half_word(
                self.channel_3_stop_wave_ram_select,
                relative_address - 0x70,
            ),
            0x72..0x74 => {
                read_byte_from_half_word(self.channel_3_length_volume, relative_address - 0x72)
            }
            0x74..0x76 => {
                read_byte_from_half_word(self.channel_3_frequency_control, relative_address - 0x74)
            }
            0x76..0x78 => {
                todo!()
            }

            0x78..0x7a => {
                read_byte_from_half_word(self.channel_4_length_envelope, relative_address - 0x78)
            }
            0x7a..0x7c => {
                todo!()
            }

            0x7c..0x7e => {
                read_byte_from_half_word(self.channel_4_frequency_control, relative_address - 0x7c)
            }
            0x7e..0x80 => {
                todo!()
            }

            0x80..0x82 => read_byte_from_half_word(
                self.control_stereo_volumne_enable,
                relative_address - 0x80,
            ),
            0x82..0x84 => {
                read_byte_from_half_word(self.control_mixing_dma, relative_address - 0x82)
            }
            0x84..0x86 => {
                read_byte_from_half_word(self.control_sound_on_off, relative_address - 0x84)
            }

            0x88..0x8a => read_byte_from_half_word(self.sound_pwm_control, relative_address - 0x88),
            0x90..0xa0 => self
                .inactive_channel_3_wave_pattern()
                .read_byte(relative_address - 0x90),
            0xa0..0xa4 => read_byte_from_word(self.channel_a_fifo, relative_address - 0xa0),
            0xa4..0xa8 => read_byte_from_word(self.channel_b_fifo, relative_address - 0xa4),
            0xa8..0xb0 => 0,

            _ => todo!("reading from {address:x}"),
        }
    }

    fn write_byte(&mut self, address: usize, byte: u8) {
        let relative_address = address & !0x4000000;
        match relative_address {
            0x60..0x62 => write_byte_to_half_word(
                &mut self.channel_1_sweep_register,
                relative_address - 0x60,
                byte,
            ),
            0x62..0x64 => write_byte_to_half_word(
                &mut self.channel_1_duty_length_envelope,
                relative_address - 0x62,
                byte,
            ),
            0x64..0x66 => write_byte_to_half_word(
                &mut self.channel_1_frequency_control,
                relative_address - 0x64,
                byte,
            ),
            0x66..0x68 => {}
            0x68..0x6a => {
                write_byte_to_half_word(
                    &mut self.channel_2_duty_length_envelope,
                    relative_address - 0x68,
                    byte,
                );
            }
            0x6a..0x6c => {}
            0x6C..0x6E => write_byte_to_half_word(
                &mut self.channel_2_frequency_control,
                relative_address - 0x6C,
                byte,
            ),
            0x6e..0x70 => {}
            0x70..0x72 => write_byte_to_half_word(
                &mut self.channel_3_stop_wave_ram_select,
                relative_address - 0x70,
                byte,
            ),
            0x72..0x74 => write_byte_to_half_word(
                &mut self.channel_3_length_volume,
                relative_address - 0x72,
                byte,
            ),
            0x74..0x76 => write_byte_to_half_word(
                &mut self.channel_3_frequency_control,
                relative_address - 0x74,
                byte,
            ),
            0x76..0x78 => {}

            0x78..0x7a => write_byte_to_half_word(
                &mut self.channel_4_length_envelope,
                relative_address - 0x78,
                byte,
            ),
            0x7a..0x7c => {}

            0x7c..0x7e => write_byte_to_half_word(
                &mut self.channel_4_frequency_control,
                relative_address - 0x7c,
                byte,
            ),
            0x7e..0x80 => {}

            0x80..0x82 => write_byte_to_half_word(
                &mut self.control_stereo_volumne_enable,
                relative_address - 0x80,
                byte,
            ),
            0x82..0x84 => {
                write_byte_to_half_word(&mut self.control_mixing_dma, relative_address - 0x82, byte)
            }
            0x84..0x86 => write_byte_to_half_word(
                &mut self.control_sound_on_off,
                relative_address - 0x84,
                byte,
            ),
            0x86..0x88 => {}
            0x88..0x8a => {
                // TODO: Only BIOS can access this!
                write_byte_to_half_word(&mut self.sound_pwm_control, relative_address - 0x88, byte);
            }
            0x8a..0x90 => {}
            0x90..0xa0 => self
                .inactive_channel_3_wave_pattern_mut()
                .write_byte(relative_address - 0x90, byte),
            0xa0..0xa4 => {
                write_byte_to_word(&mut self.channel_a_fifo, relative_address - 0xa0, byte)
            }
            0xa4..0xa8 => {
                write_byte_to_word(&mut self.channel_b_fifo, relative_address - 0xa4, byte)
            }
            0xa8..0xb0 => {}
            _ => todo!("writing {byte:x} to {address:x}"),
        }
    }
}
