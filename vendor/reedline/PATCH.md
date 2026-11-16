This directory contains the library sources, license, README and normalized manifest
from the crates.io reedline 0.42.0 package. It is a dependency package, not code
from a later Nushell tree. The only functional modification is raising its
optional rusqlite dependency from 0.31 to 0.37, so SQLite-backed history can link
with nu-command's rusqlite 0.37. The manifest omits example targets whose sources
are not vendored. Remove this patch when a compatible reedline release is adopted.
