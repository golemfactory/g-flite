use hound;

fn main() {
    let a = hound::WavReader::open("out/a.wav").unwrap();
    let b = hound::WavReader::open("out/b.wav").unwrap();
    let spec = a.spec();

    let mut concat: Vec<i16> = Vec::new();
    concat.extend(a.into_samples::<i16>().map(|s| s.unwrap()));
    concat.extend(b.into_samples::<i16>().map(|s| s.unwrap()));

    let mut writer = hound::WavWriter::create("out/combined.wav", spec).unwrap();
    for sample in concat {
        writer.write_sample(sample).unwrap();
    }
}
