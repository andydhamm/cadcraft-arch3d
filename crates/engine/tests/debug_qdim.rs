use cadcraft_doc::{
    Handle,
    arch::{ARCH_HANDLE_BASE, arch_handle, is_arch_handle},
};

#[test]
fn handle_parse_hex_keeps_ordinary_uppercase_handles() {
    assert_eq!(Handle::parse_hex("A00"), Some(Handle(0xA00)));
    assert_eq!(Handle::parse_hex("A0000001"), Some(Handle(0xA0000001)));
    assert!(!is_arch_handle(Handle::parse_hex("A0000001").unwrap()));
    assert_eq!(Handle::parse_hex("a0000001"), Some(Handle(ARCH_HANDLE_BASE | 1)));
    assert_eq!(Handle::parse_hex("a0000001").unwrap(), arch_handle(cadcraft_doc::arch::ArchId(1)));
}
