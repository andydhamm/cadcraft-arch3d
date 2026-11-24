//! A DXF entity whose handle falls in the range the arch proxies use keeps its
//! handle (a file's handles keep their identity), and still reads as one entity.

use cadcraft_doc::Handle;

const DXF_RESERVED_HANDLE: &str = "\
0\nSECTION\n2\nENTITIES\n\
0\nLINE\n5\nFFFFFFFF00000005\n8\n0\n\
10\n0.0\n20\n0.0\n30\n0.0\n\
11\n10.0\n21\n0.0\n31\n0.0\n\
0\nENDSEC\n0\nEOF\n";

#[test]
fn a_file_handle_in_the_arch_range_keeps_its_identity() {
    let d = cadcraft_io::read_dxf(DXF_RESERVED_HANDLE.as_bytes()).unwrap();
    let entities: Vec<_> = d.model.iter().collect();
    assert_eq!(entities.len(), 1, "expected one entity after reading");
    assert_eq!(entities[0].handle, Handle(0xFFFF_FFFF_0000_0005));
}
