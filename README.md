# One billion row challenge in Rust.

This is a solution to the [1 billion row challenge](https://www.morling.dev/blog/one-billion-row-challenge/) in Rust ([link to the github repo](https://github.com/gunnarmorling/1brc)).

In order to run it you need a one billion row measurements.txt file. There's some
help on that at the end of this README.

Compile:

```
cargo build --release
```

Before you run it, make sure the measurements file is cached in memory
(replace `../1brc/measurements.txt` with the actual location if it's different):

```
dd if=../1brc/measurements.txt of=/dev/null bs=1M
dd if=../1brc/measurements.txt of=/dev/null bs=1M
```

Then run it, maybe a few times to make sure all caches are warm:

```
time target/release/onebrc-rs < ../1brc//measurements.txt
time target/release/onebrc-rs < ../1brc//measurements.txt
```

On a macbook M1 Pro 2021, it runs in 3.1 seconds.

## Generating the measurements.txt file.

To start you need to generate the measurements.txt file. There are scripts for this in
the original github repo, but the project needs java 21.

To install java 21 on macos:

```
brew install openjdk@21
export PATH=/opt/homebrew/opt/openjdk@21/bin:$PATH
```

Then you need to build the project by running the `mvnw` script from the github repo.
I had some trouble with this on macos and had to patch the script like this:

```
--- a/mvnw
+++ b/mvnw
@@ -96,12 +96,12 @@ if [ -z "$JAVA_HOME" ]; then
     readLink=$(which readlink)
     if [ ! "$(expr "$readLink" : '\([^ ]*\)')" = "no" ]; then
       if $darwin ; then
-        javaHome="$(dirname "\"$javaExecutable\"")"
-        javaExecutable="$(cd "\"$javaHome\"" && pwd -P)/javac"
+        javaHome=$(dirname $javaExecutable)
+        javaExecutable=$(cd $javaHome && pwd -P)/javac
       else
         javaExecutable="$(readlink -f "\"$javaExecutable\"")"
       fi
-      javaHome="$(dirname "\"$javaExecutable\"")"
+      javaHome="$(dirname $javaExecutable)"
       javaHome=$(expr "$javaHome" : '\(.*\)/bin')
       JAVA_HOME="$javaHome"
       export JAVA_HOME
```

Build the code:

```
./mvnw clean verify
```

Finally generate the measurements.txt file:

```
/create_measurements.sh 1000000000
```
