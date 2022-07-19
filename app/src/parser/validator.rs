/*******************************************************************************
*   (c) 2021 Zondax GmbH
*
*  Licensed under the Apache License, Version 2.0 (the "License");
*  you may not use this file except in compliance with the License.
*  You may obtain a copy of the License at
*
*      http://www.apache.org/licenses/LICENSE-2.0
*
*  Unless required by applicable law or agreed to in writing, software
*  distributed under the License is distributed on an "AS IS" BASIS,
*  WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
*  See the License for the specific language governing permissions and
*  limitations under the License.
********************************************************************************/
use crate::handlers::handle_ui_message;
use crate::parser::{
    intstr_to_fpstr_inplace, DisplayableItem, FromBytes, ParserError, NANO_AVAX_DECIMAL_DIGITS,
};
use crate::sys::{bech32, hash::Ripemd160};
use core::{mem::MaybeUninit, ptr::addr_of_mut};
use nom::{
    bytes::complete::take,
    number::complete::{be_i64, be_u64},
    sequence::tuple,
};
use zemu_sys::ViewError;

pub const NODE_ID_LEN: usize = 20;
const PREFIX_NODE_ID_LEN: usize = 7;
const NODE_ID_PREFIX: &[u8] = b"NodeId-";

#[derive(Clone, Copy, PartialEq)]
#[repr(C)]
#[cfg_attr(test, derive(Debug))]
pub struct Validator<'b> {
    pub node_id: &'b [u8; NODE_ID_LEN],
    pub start_time: i64,
    pub endtime: i64,
    pub weight: u64,
}

impl<'b> FromBytes<'b> for Validator<'b> {
    #[inline(never)]
    fn from_bytes_into(
        input: &'b [u8],
        out: &mut MaybeUninit<Self>,
    ) -> Result<&'b [u8], nom::Err<ParserError>> {
        crate::sys::zemu_log_stack("Validator::from_bytes_into\x00");

        let (rem, node_id) = take(NODE_ID_LEN)(input)?;
        let node_id = arrayref::array_ref!(node_id, 0, NODE_ID_LEN);

        let (rem, (start_time, endtime, weight)) = tuple((be_i64, be_i64, be_u64))(rem)?;

        // check for appropiate timestamps
        if !(endtime > start_time) {
            return Err(ParserError::InvalidTimestamp.into());
        }

        let out = out.as_mut_ptr();
        unsafe {
            addr_of_mut!((*out).node_id).write(node_id);
            addr_of_mut!((*out).start_time).write(start_time);
            addr_of_mut!((*out).endtime).write(endtime);
            addr_of_mut!((*out).weight).write(weight);
        }

        Ok(rem)
    }
}

impl<'b> DisplayableItem for Validator<'b> {
    fn num_items(&self) -> usize {
        // node_id, start_time, endtime and total_stake
        4
    }

    fn render_item(
        &self,
        item_n: u8,
        title: &mut [u8],
        message: &mut [u8],
        page: u8,
    ) -> Result<u8, zemu_sys::ViewError> {
        use crate::parser::timestamp_to_str_date;
        use bolos::{pic_str, PIC};
        use lexical_core::{write as itoa, Number};

        match item_n {
            0 => {
                let label = pic_str!(b"Validator");
                title[..label.len()].copy_from_slice(label);

                // format the node_id
                let prefix = PIC::new(NODE_ID_PREFIX).into_inner();
                const prefix_len: usize = NODE_ID_PREFIX.len();

                const MAX_SIZE: usize =
                    bech32::estimate_size(0, Ripemd160::DIGEST_LEN) + prefix_len;

                let mut node_id = [0; MAX_SIZE];

                node_id[..prefix.len()].copy_from_slice(prefix);

                let len = bech32::encode(
                    "",
                    &self.node_id,
                    &mut node_id[prefix_len..],
                    bech32::Variant::Bech32,
                )
                .map_err(|_| ViewError::Unknown)?
                    + prefix_len;

                handle_ui_message(&node_id[..len], message, page)
            }
            1 => {
                let label = pic_str!(b"Start time");
                title[..label.len()].copy_from_slice(label);
                let time =
                    timestamp_to_str_date(self.start_time).map_err(|_| ViewError::Unknown)?;
                handle_ui_message(time.as_slice(), message, page)
            }
            2 => {
                let label = pic_str!(b"End time");
                title[..label.len()].copy_from_slice(label);
                let time = timestamp_to_str_date(self.endtime).map_err(|_| ViewError::Unknown)?;
                handle_ui_message(time.as_slice(), message, page)
            }
            3 => {
                let label = pic_str!(b"Total stake(AVAX)");
                title[..label.len()].copy_from_slice(label);

                let mut buffer = [0; u64::FORMATTED_SIZE + 2];

                itoa(self.weight, &mut buffer);

                let buffer = intstr_to_fpstr_inplace(&mut buffer[..], NANO_AVAX_DECIMAL_DIGITS)
                    .map_err(|_| ViewError::Unknown)?;

                handle_ui_message(buffer, message, page)
            }

            _ => Err(ViewError::NoData),
        }
    }
}
