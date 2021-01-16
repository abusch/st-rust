// This file is generated by rust-protobuf 2.18.1. Do not edit
// @generated

// https://github.com/rust-lang/rust-clippy/issues/702
#![allow(unknown_lints)]
#![allow(clippy::all)]

#![allow(unused_attributes)]
#![rustfmt::skip]

#![allow(box_pointers)]
#![allow(dead_code)]
#![allow(missing_docs)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(non_upper_case_globals)]
#![allow(trivial_casts)]
#![allow(unused_imports)]
#![allow(unused_results)]
//! Generated file from `local.proto`

/// Generated files are compatible only with the same version
/// of protobuf runtime.
// const _PROTOBUF_VERSION_CHECK: () = ::protobuf::VERSION_2_18_1;

#[derive(PartialEq,Clone,Default)]
pub struct Announce {
    // message fields
    pub id: ::std::vec::Vec<u8>,
    pub addresses: ::protobuf::RepeatedField<::std::string::String>,
    pub instance_id: i64,
    // special fields
    pub unknown_fields: ::protobuf::UnknownFields,
    pub cached_size: ::protobuf::CachedSize,
}

impl<'a> ::std::default::Default for &'a Announce {
    fn default() -> &'a Announce {
        <Announce as ::protobuf::Message>::default_instance()
    }
}

impl Announce {
    pub fn new() -> Announce {
        ::std::default::Default::default()
    }

    // bytes id = 1;


    pub fn get_id(&self) -> &[u8] {
        &self.id
    }
    pub fn clear_id(&mut self) {
        self.id.clear();
    }

    // Param is passed by value, moved
    pub fn set_id(&mut self, v: ::std::vec::Vec<u8>) {
        self.id = v;
    }

    // Mutable pointer to the field.
    // If field is not initialized, it is initialized with default value first.
    pub fn mut_id(&mut self) -> &mut ::std::vec::Vec<u8> {
        &mut self.id
    }

    // Take field
    pub fn take_id(&mut self) -> ::std::vec::Vec<u8> {
        ::std::mem::replace(&mut self.id, ::std::vec::Vec::new())
    }

    // repeated string addresses = 2;


    pub fn get_addresses(&self) -> &[::std::string::String] {
        &self.addresses
    }
    pub fn clear_addresses(&mut self) {
        self.addresses.clear();
    }

    // Param is passed by value, moved
    pub fn set_addresses(&mut self, v: ::protobuf::RepeatedField<::std::string::String>) {
        self.addresses = v;
    }

    // Mutable pointer to the field.
    pub fn mut_addresses(&mut self) -> &mut ::protobuf::RepeatedField<::std::string::String> {
        &mut self.addresses
    }

    // Take field
    pub fn take_addresses(&mut self) -> ::protobuf::RepeatedField<::std::string::String> {
        ::std::mem::replace(&mut self.addresses, ::protobuf::RepeatedField::new())
    }

    // int64 instance_id = 3;


    pub fn get_instance_id(&self) -> i64 {
        self.instance_id
    }
    pub fn clear_instance_id(&mut self) {
        self.instance_id = 0;
    }

    // Param is passed by value, moved
    pub fn set_instance_id(&mut self, v: i64) {
        self.instance_id = v;
    }
}

impl ::protobuf::Message for Announce {
    fn is_initialized(&self) -> bool {
        true
    }

    fn merge_from(&mut self, is: &mut ::protobuf::CodedInputStream<'_>) -> ::protobuf::ProtobufResult<()> {
        while !is.eof()? {
            let (field_number, wire_type) = is.read_tag_unpack()?;
            match field_number {
                1 => {
                    ::protobuf::rt::read_singular_proto3_bytes_into(wire_type, is, &mut self.id)?;
                },
                2 => {
                    ::protobuf::rt::read_repeated_string_into(wire_type, is, &mut self.addresses)?;
                },
                3 => {
                    if wire_type != ::protobuf::wire_format::WireTypeVarint {
                        return ::std::result::Result::Err(::protobuf::rt::unexpected_wire_type(wire_type));
                    }
                    let tmp = is.read_int64()?;
                    self.instance_id = tmp;
                },
                _ => {
                    ::protobuf::rt::read_unknown_or_skip_group(field_number, wire_type, is, self.mut_unknown_fields())?;
                },
            };
        }
        ::std::result::Result::Ok(())
    }

    // Compute sizes of nested messages
    #[allow(unused_variables)]
    fn compute_size(&self) -> u32 {
        let mut my_size = 0;
        if !self.id.is_empty() {
            my_size += ::protobuf::rt::bytes_size(1, &self.id);
        }
        for value in &self.addresses {
            my_size += ::protobuf::rt::string_size(2, &value);
        };
        if self.instance_id != 0 {
            my_size += ::protobuf::rt::value_size(3, self.instance_id, ::protobuf::wire_format::WireTypeVarint);
        }
        my_size += ::protobuf::rt::unknown_fields_size(self.get_unknown_fields());
        self.cached_size.set(my_size);
        my_size
    }

    fn write_to_with_cached_sizes(&self, os: &mut ::protobuf::CodedOutputStream<'_>) -> ::protobuf::ProtobufResult<()> {
        if !self.id.is_empty() {
            os.write_bytes(1, &self.id)?;
        }
        for v in &self.addresses {
            os.write_string(2, &v)?;
        };
        if self.instance_id != 0 {
            os.write_int64(3, self.instance_id)?;
        }
        os.write_unknown_fields(self.get_unknown_fields())?;
        ::std::result::Result::Ok(())
    }

    fn get_cached_size(&self) -> u32 {
        self.cached_size.get()
    }

    fn get_unknown_fields(&self) -> &::protobuf::UnknownFields {
        &self.unknown_fields
    }

    fn mut_unknown_fields(&mut self) -> &mut ::protobuf::UnknownFields {
        &mut self.unknown_fields
    }

    fn as_any(&self) -> &dyn (::std::any::Any) {
        self as &dyn (::std::any::Any)
    }
    fn as_any_mut(&mut self) -> &mut dyn (::std::any::Any) {
        self as &mut dyn (::std::any::Any)
    }
    fn into_any(self: ::std::boxed::Box<Self>) -> ::std::boxed::Box<dyn (::std::any::Any)> {
        self
    }

    fn descriptor(&self) -> &'static ::protobuf::reflect::MessageDescriptor {
        Self::descriptor_static()
    }

    fn new() -> Announce {
        Announce::new()
    }

    fn descriptor_static() -> &'static ::protobuf::reflect::MessageDescriptor {
        static descriptor: ::protobuf::rt::LazyV2<::protobuf::reflect::MessageDescriptor> = ::protobuf::rt::LazyV2::INIT;
        descriptor.get(|| {
            let mut fields = ::std::vec::Vec::new();
            fields.push(::protobuf::reflect::accessor::make_simple_field_accessor::<_, ::protobuf::types::ProtobufTypeBytes>(
                "id",
                |m: &Announce| { &m.id },
                |m: &mut Announce| { &mut m.id },
            ));
            fields.push(::protobuf::reflect::accessor::make_repeated_field_accessor::<_, ::protobuf::types::ProtobufTypeString>(
                "addresses",
                |m: &Announce| { &m.addresses },
                |m: &mut Announce| { &mut m.addresses },
            ));
            fields.push(::protobuf::reflect::accessor::make_simple_field_accessor::<_, ::protobuf::types::ProtobufTypeInt64>(
                "instance_id",
                |m: &Announce| { &m.instance_id },
                |m: &mut Announce| { &mut m.instance_id },
            ));
            ::protobuf::reflect::MessageDescriptor::new_pb_name::<Announce>(
                "Announce",
                fields,
                file_descriptor_proto()
            )
        })
    }

    fn default_instance() -> &'static Announce {
        static instance: ::protobuf::rt::LazyV2<Announce> = ::protobuf::rt::LazyV2::INIT;
        instance.get(Announce::new)
    }
}

impl ::protobuf::Clear for Announce {
    fn clear(&mut self) {
        self.id.clear();
        self.addresses.clear();
        self.instance_id = 0;
        self.unknown_fields.clear();
    }
}

impl ::std::fmt::Debug for Announce {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        ::protobuf::text_format::fmt(self, f)
    }
}

impl ::protobuf::reflect::ProtobufValue for Announce {
    fn as_ref(&self) -> ::protobuf::reflect::ReflectValueRef {
        ::protobuf::reflect::ReflectValueRef::Message(self)
    }
}

static file_descriptor_proto_data: &'static [u8] = b"\
    \n\x0blocal.proto\x12\x08discover\"Y\n\x08Announce\x12\x0e\n\x02id\x18\
    \x01\x20\x01(\x0cR\x02id\x12\x1c\n\taddresses\x18\x02\x20\x03(\tR\taddre\
    sses\x12\x1f\n\x0binstance_id\x18\x03\x20\x01(\x03R\ninstanceIdb\x06prot\
    o3\
";

static file_descriptor_proto_lazy: ::protobuf::rt::LazyV2<::protobuf::descriptor::FileDescriptorProto> = ::protobuf::rt::LazyV2::INIT;

fn parse_descriptor_proto() -> ::protobuf::descriptor::FileDescriptorProto {
    ::protobuf::parse_from_bytes(file_descriptor_proto_data).unwrap()
}

pub fn file_descriptor_proto() -> &'static ::protobuf::descriptor::FileDescriptorProto {
    file_descriptor_proto_lazy.get(|| {
        parse_descriptor_proto()
    })
}