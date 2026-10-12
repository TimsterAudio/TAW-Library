#!/usr/bin/env bash
# Builds a deliberately messy test library in tools/fixtures/ (git-ignored),
# for trying Music Box without your own music. Every track is silence, so
# the files stay small. Needs ffmpeg with libmp3lame.
#
#   fixtures/Data/       the main "drive": tagged albums with every kind of gap
#   fixtures/OLD IPOD/   an old iPod's scrambled file names
#   fixtures/Data/Samples, fixtures/Data/Projects/Song   left out by the scanner
set -euo pipefail
cd "$(dirname "$0")"
F=fixtures
rm -rf "$F"
q="-loglevel error -y"

art() { # file size colour1 colour2
  ffmpeg $q -f lavfi -i "gradients=s=$2x$2:c0=$3:c1=$4:x0=0:y0=0:x1=$2:y1=$2:d=1" -frames:v 1 "$1"
}

# track dir ext seconds title [extra ffmpeg metadata ...]
track() {
  local dir=$1 file=$2 secs=$3; shift 3
  mkdir -p "$dir"
  local codec=()
  case "$file" in
    *.flac) codec=(-c:a flac -sample_fmt s16) ;;
    *.mp3) codec=(-c:a libmp3lame -q:a 9 -id3v2_version 3) ;;
    *.m4a) codec=(-c:a aac -b:a 32k) ;;
    *.wav) codec=(-c:a pcm_s16le) ;;
  esac
  local input=(-f lavfi -t "$secs" -i "anullsrc=r=44100:cl=stereo")
  if [ -n "${ART:-}" ]; then
    ffmpeg $q "${input[@]}" -i "$ART" -map 0 -map 1 -c:v mjpeg -disposition:v attached_pic \
      "${codec[@]}" "$@" "$dir/$file"
  else
    ffmpeg $q "${input[@]}" "${codec[@]}" "$@" "$dir/$file"
  fi
}

# album dir ext year(or -) album_artist(or -) artist album "secs|title" ...
album() {
  local dir=$1 ext=$2 year=$3 aa=$4 artist=$5 alb=$6; shift 6
  local n=0
  for t in "$@"; do
    n=$((n + 1))
    local secs=${t%%|*} title=${t#*|}
    local md=(-metadata "title=$title" -metadata "artist=$artist" -metadata "album=$alb" -metadata "track=$n")
    [ "$year" != - ] && md+=(-metadata "date=$year")
    [ "$aa" != - ] && md+=(-metadata "album_artist=$aa")
    track "$dir" "$(printf %02d $n) ${title//\//-}.$ext" "$secs" "${md[@]}"
  done
}

D="$F/Data/Music"
mkdir -p "$F/art"
art "$F/art/okc.jpg" 1400 0xdfe3e6 0x6e8fa8
art "$F/art/amn.jpg" 300 0xb33a2c 0x1d1d1d
art "$F/art/dummy.jpg" 1000 0x1e3b4a 0xc9d6d9

ART="$F/art/okc.jpg" album "$D/Radiohead/OK Computer" flac 1997 Radiohead Radiohead "OK Computer" \
  "284|Airbag" "383|Paranoid Android" "267|Subterranean Homesick Alien" "264|Exit Music (For a Film)" \
  "299|Let Down" "261|Karma Police" "117|Fitter Happier" "230|Electioneering" "285|Climbing Up the Walls" \
  "228|No Surprises" "259|Lucky" "324|The Tourist"
album "$D/Radiohead - The Bends" mp3 - Radiohead Radiohead "The Bends" \
  "238|Planet Telex" "229|The Bends" "273|High and Dry" "269|Fake Plastic Trees" "198|Bones" \
  "247|(Nice Dream)" "212|Just" "306|My Iron Lung" "245|Bullet Proof..I Wish I Was" "277|Black Star" \
  "201|Sulk" "268|Street Spirit (Fade Out)"
art "$D/Radiohead - The Bends/folder.jpg" 600 0x5c6f7a 0xc9b48a
album "$D/Radiohead/Kid A" flac 2000 Radiohead Radiohead "Kid A" \
  "290|Everything in Its Right Place" "240|Kid A" "347|The National Anthem" "266|How to Disappear Completely" \
  "106|Treefingers" "250|Optimistic" "248|In Limbo" "256|Idioteque" "331|Morning Bell" "421|Motion Picture Soundtrack"
art "$D/Radiohead/Kid A/cover.jpg" 1200 0x7e2f2a 0xe9e4dd
ART="$F/art/amn.jpg" album "$D/iTunes/Radiohead/Amnesiac" m4a 2001 Radiohead Radiohead "Amnesiac" \
  "254|Packt Like Sardines in a Crushd Tin Box" "303|Pyramid Song" "238|Pulk/Pull Revolving Doors" \
  "214|You and Whose Army?" "302|I Might Be Wrong" "264|Knives Out" "216|Morning Bell/Amnesiac" \
  "122|Dollars and Cents" "240|Hunting Bears" "301|Like Spinning Plates" "282|Life in a Glasshouse"
# The King of Limbs: WAV with no tags at all.
for t in "Bloom" "Morning Mr Magpie" "Little by Little" "Feral" "Lotus Flower" "Codex" "Give Up the Ghost" "Separator"; do
  track "$F/Data/Music/TKOL wav" "$t.wav" 20
done
ART="$F/art/dummy.jpg" album "$D/Portishead/Dummy" flac 1994 Portishead Portishead "Dummy" \
  "302|Mysterons" "251|Sour Times" "238|Strangers" "260|It Could Be Sweet" "291|Wandering Star" \
  "229|It's a Fire" "234|Numb" "305|Roads" "219|Pedestal" "301|Biscuit" "306|Glory Box"
album "$D/Portishead (1997)" mp3 1997 - Portishead "Portishead" \
  "273|Cowboys" "286|All Mine" "298|Undenied" "289|Half Day Closing" "282|Over" "235|Humming" \
  "234|Mourning Air" "238|Seven Months" "333|Only You" "285|Elysium" "355|Western Eyes"
# The same artist written two ways, and a two-disc set with no disc numbers.
album "$D/The Beatles/Abbey Road" mp3 1969 "The Beatles" "The Beatles" "Abbey Road" \
  "259|Come Together" "183|Something" "207|Maxwell's Silver Hammer" "206|Oh! Darling"
album "$D/Beatles/White Album/CD1" mp3 1968 - Beatles "The White Album" \
  "163|Back in the U.S.S.R." "176|Dear Prudence" "169|Glass Onion"
album "$D/Beatles/White Album/CD2" mp3 1968 - Beatles "The White Album" \
  "170|Birthday" "196|Yer Blues" "148|Mother Nature's Son"
# A compilation with no album artist.
D2="$D/Compilations/Late Night Mix"
track "$D2" "01 Teardrop.mp3" 330 -metadata "title=Teardrop" -metadata "artist=Massive Attack" -metadata "album=Late Night Mix" -metadata track=1
track "$D2" "02 Glory Box.mp3" 306 -metadata "title=Glory Box" -metadata "artist=Portishead" -metadata "album=Late Night Mix" -metadata track=2
track "$D2" "03 Roygbiv.mp3" 151 -metadata "title=Roygbiv" -metadata "artist=Boards of Canada" -metadata "album=Late Night Mix"

# Left out by the scanner: a sample pack and a DAW project.
for s in Kick Snare Hat Clap; do track "$F/Data/Samples/808 Pack" "$s.wav" 1; done
track "$F/Data/Projects/Song" "Bounce.wav" 5
echo x > "$F/Data/Projects/Song/Song.als"

# An old iPod: scrambled names, tags intact.
I="$F/OLD IPOD/iPod_Control/Music/F07"
n=0
for t in "You" "Creep" "How Do You?" "Stop Whispering" "Thinking About You"; do
  n=$((n + 1))
  track "$I" "$(printf 'XKQ%c.mp3' $(printf "\\x$(printf %x $((64 + n)))"))" 200 \
    -metadata "title=$t" -metadata "artist=Radiohead" -metadata "album=Pablo Honey" -metadata "date=1993" -metadata "track=$n"
done
rm -rf "$F/art"
echo "Made $(find "$F" -type f | wc -l) files in tools/$F"
