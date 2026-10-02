use crate::bucket::Bucket;

/// Whether a run uploads. A dry run reads the bucket's index when it has one, else starts
/// from an empty index; an upload always has a bucket.
pub enum Mode {
    DryRun(Option<Bucket>),
    Upload(Bucket),
}
