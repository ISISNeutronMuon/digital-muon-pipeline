# `digitiser-aggregator`

The digitiser aggregator is responsible for aggregating a full instruments worth of data into a single message.
Currently this is only possible for event data.

Frames are uniquely identified by the complete metadata struct, which is entirely derived from the status packet so should be identical across all digitisers.

## Failure detection

Frames are given a TTL, in which all expected digitiers must deliver their messages for the given frame.
This timeout begins when the first message for a given frame is received.

Incomplete frames are released after this timeout expires, with only the data that has been received.
