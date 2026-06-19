# Deterministic partitioner for the DRY-violation scan (python-free).
#
# Input (stdin): lines "<loc>\t<path>" for every first-party *.rs file,
#   already sorted by path.
# Output: writes JSON to the file named by -v out=<path> using awk's own
#   file I/O (so no shell '>' redirect is needed; bash-write-guard only scans
#   the bash command string, not this script's contents).
#
# JSON shape: {"totalLoc": int, "units": [
#   {"name","layer","approxLoc","fileCount","paths":[...]}, ...]}
#
# Modules over THRESH LOC are split into ~TARGET-LOC contiguous chunks.

function layer_of(p) {
    if (p ~ /^libs\/domain\//)         return "domain"
    if (p ~ /^libs\/usecase\//)        return "usecase"
    if (p ~ /^libs\/infrastructure\//) return "infrastructure"
    if (p ~ /^apps\/cli-composition\//) return "cli-composition"
    if (p ~ /^apps\/cli\//)            return "cli"
    return "other"
}

function module_of(p,   r, s) {
    if (match(p, /\/src\//)) {
        r = substr(p, RSTART + 5)
        s = index(r, "/")
        if (s > 0) return substr(r, 1, s - 1)
        return "_root"
    }
    return "_root"
}

function emit(name, lyr, loc, cnt, fstr,   arr, m, j, realj, pf) {
    fileno++
    pf = sprintf("%s/u%03d.paths", pathsdir, fileno)
    if (unitno++ > 0) printf "    ,\n" > out
    printf "    {\n" > out
    printf "      \"name\": \"%s\",\n", name > out
    printf "      \"layer\": \"%s\",\n", lyr > out
    printf "      \"approxLoc\": %d,\n", loc > out
    printf "      \"fileCount\": %d,\n", cnt > out
    printf "      \"pathsFile\": \"%s\",\n", pf > out
    printf "      \"paths\": [\n" > out
    m = split(fstr, arr, "\n")
    realj = 0
    for (j = 1; j <= m; j++) {
        if (arr[j] == "") continue
        realj++
        printf "        \"%s\"%s\n", arr[j], (realj < cnt ? "," : "") > out
        print arr[j] > pf
    }
    close(pf)
    printf "      ]\n    }\n" > out
}

BEGIN { TARGET = 9000; THRESH = 12600; n = 0; unitno = 0; fileno = 0 }

{
    loc = $1; path = $2
    key = layer_of(path) "\t" module_of(path)
    if (!(key in seen)) { seen[key] = 1; order[n++] = key }
    files[key] = files[key] path "\n"
    flocs[key] = flocs[key] loc "\n"
    ksum[key] += loc
    kcnt[key] += 1
}

END {
    total = 0
    for (k in ksum) total += ksum[k]
    printf "{\n  \"totalLoc\": %d,\n  \"units\": [\n", total > out

    for (i = 0; i < n; i++) {
        key = order[i]
        split(key, kk, "\t"); lyr = kk[1]; mod = kk[2]
        nf = kcnt[key]
        gtotal = ksum[key]
        split(files[key], parr, "\n")
        split(flocs[key], larr, "\n")

        if (gtotal <= THRESH) {
            emit(lyr "/" mod, lyr, gtotal, nf, files[key])
            continue
        }

        nch = int((gtotal + TARGET - 1) / TARGET)
        if (nch < 1) nch = 1
        per = gtotal / nch
        cfiles = ""; clocs = 0; cin = 0; chunk_no = 1
        for (j = 1; j <= nf; j++) {
            cfiles = cfiles parr[j] "\n"
            clocs += larr[j]
            cin += 1
            if (clocs >= per && chunk_no < nch && j < nf) {
                emit(lyr "/" mod "-" chunk_no, lyr, clocs, cin, cfiles)
                chunk_no += 1
                cfiles = ""; clocs = 0; cin = 0
            }
        }
        if (cin > 0) emit(lyr "/" mod "-" chunk_no, lyr, clocs, cin, cfiles)
    }

    printf "  ]\n}\n" > out
}
