/*
 * Pure-C reproduction of the 3Delight interactive-render teardown crash.
 *
 * Shape: N interactive+progressive contexts render a trivial scene, are
 * stopped and waited on, NSIEnd'd -- and then the process exits. 3Delight
 * finishes tearing down on detached threads; exiting out from under them
 * segfaults AFTER every render has completed cleanly.
 *
 * argv[1] = milliseconds to sleep before returning from main (default 0).
 *           0 reproduces; a few hundred ms does not.
 * argv[2] = number of contexts (default 2).
 *
 * Build:  gcc -O0 -g repro.c -o repro -I$DELIGHT/include -L$DELIGHT/lib -l3delight
 * Run:    ./repro 0 2   ; echo $?      -> 139 (SIGSEGV)
 *         ./repro 500 2 ; echo $?      -> 0
 */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <pthread.h>
#include <dirent.h>

/* Number of live threads in this process. */
static int nthreads(void)
{
    DIR *d = opendir("/proc/self/task");
    if (!d) return -1;
    int n = 0;
    struct dirent *e;
    while ((e = readdir(d)))
        if (e->d_name[0] != '.') ++n;
    closedir(d);
    return n;
}
#include "nsi.h"
#include "ndspy.h"

#define MAX_CTX 8

/* ------------------------------------------------------------------ *
 * An in-process display driver, registered the way nsi-rs registers
 * its FERRIS drivers (DspyRegisterDriver). This is the ingredient the
 * "exr" driver does not have: 3Delight calls back into *this binary*,
 * including while it tears down.
 * ------------------------------------------------------------------ */
static int g_buckets = 0;

static PtDspyError crepro_open(
    PtDspyImageHandle *image, const char *drivername, const char *filename,
    int width, int height, int paramCount, const UserParameter *parameters,
    int iFormatCount, PtDspyDevFormat *format, PtFlagStuff *flagstuff)
{
    fprintf(stderr, "crepro_open: drv=%s file=%s %dx%d fmts=%d\n", drivername, filename, width, height, iFormatCount);
    (void)drivername; (void)filename; (void)paramCount; (void)parameters;
    (void)width; (void)height;
    for (int i = 0; i < iFormatCount; ++i)
        format[i].type = PkDspyFloat32;
    *image = (PtDspyImageHandle)malloc(16);
    flagstuff->flags &= ~PkDspyFlagsWantsEmptyBuckets;
    return PkDspyErrorNone;
}

static PtDspyError crepro_write(
    PtDspyImageHandle image, int xmin, int xmax, int ymin, int ymax,
    int entrysize, const unsigned char *data)
{
    (void)image; (void)xmin; (void)xmax; (void)ymin; (void)ymax;
    (void)entrysize; (void)data;
    __sync_fetch_and_add(&g_buckets, 1);
    return PkDspyErrorNone;
}

static PtDspyError crepro_close(PtDspyImageHandle image)
{
    fprintf(stderr, "crepro_close\n");
    free(image);
    return PkDspyErrorNone;
}

static PtDspyError crepro_query(
    PtDspyImageHandle image, PtDspyQueryType type, int len, void *data)
{
    (void)image;
    /* Answer the same queries FERRIS answers; refusing these is why the
       renderer delivered no buckets. */
    if (type == PkProgressiveQuery && data && len >= (int)sizeof(PtDspyProgressiveInfo)) {
        ((PtDspyProgressiveInfo *)data)->acceptProgressive = 1;
        return PkDspyErrorNone;
    }
    if (type == PkThreadQuery && data && len >= (int)sizeof(PtDspyThreadInfo)) {
        ((PtDspyThreadInfo *)data)->multithread = 1;
        return PkDspyErrorNone;
    }
    return PkDspyErrorUnsupported;
}


static NSIContext_t build(int i, char *filename)
{
    NSIContext_t ctx = NSIBegin(0, NULL);

    /* Camera with a transform, as the Rust repro has. */
    NSICreate(ctx, "camera_xform", "transform", 0, NULL);
    NSIConnect(ctx, "camera_xform", "", NSI_SCENE_ROOT, "objects", 0, NULL);
    double xf[16] = { 1,0,0,0, 0,1,0,0, 0,0,1,0, 0,0,5,1 };
    struct NSIParam_t xp = { "transformationmatrix", xf,
                             NSITypeDoubleMatrix, 0, 1, 0 };
    NSISetAttribute(ctx, "camera_xform", 1, &xp);

    NSICreate(ctx, "cam", "perspectivecamera", 0, NULL);
    NSIConnect(ctx, "cam", "", "camera_xform", "objects", 0, NULL);
    float fov = 45.f;
    struct NSIParam_t fp = { "fov", &fov, NSITypeFloat, 0, 1, 0 };
    NSISetAttribute(ctx, "cam", 1, &fp);

    NSICreate(ctx, "screen", "screen", 0, NULL);
    int res[2] = { 64, 64 };
    int oversampling = 4;
    struct NSIParam_t sp[2] = {
        /* NSIParamIsArray == 1; without it 3Delight sees a scalar int. */
        { "resolution",   res,           NSITypeInteger, 2, 1, NSIParamIsArray },
        { "oversampling", &oversampling, NSITypeInteger, 0, 1, 0 },
    };
    NSISetAttribute(ctx, "screen", 2, sp);
    NSIConnect(ctx, "screen", "", "cam", "screens", 0, NULL);

    NSICreate(ctx, "layer", "outputlayer", 0, NULL);
    const char *var = "Ci", *sf = "float";
    int withalpha = 1;
    double filterwidth = 1.0;
    struct NSIParam_t lp[4] = {
        { "variablename", &var,          NSITypeString,  0, 1, 0 },
        { "scalarformat", &sf,           NSITypeString,  0, 1, 0 },
        { "withalpha",    &withalpha,    NSITypeInteger, 0, 1, 0 },
        { "filterwidth",  &filterwidth,  NSITypeDouble,  0, 1, 0 },
    };
    NSISetAttribute(ctx, "layer", 4, lp);
    NSIConnect(ctx, "layer", "", "screen", "outputlayers", 0, NULL);

    /* An environment dome with a constant shader, so there is real shading
       work for the denoiser to chew on. */
    NSICreate(ctx, "env", "environment", 0, NULL);
    NSIConnect(ctx, "env", "", NSI_SCENE_ROOT, "objects", 0, NULL);
    NSICreate(ctx, "env_attrib", "attributes", 0, NULL);
    NSIConnect(ctx, "env_attrib", "", "env", "geometryattributes", 0, NULL);
    NSICreate(ctx, "env_shader", "shader", 0, NULL);
    NSIConnect(ctx, "env_shader", "", "env_attrib", "surfaceshader", 0, NULL);
    {
        char osl[512];
        snprintf(osl, sizeof osl, "%s/osl/dlConstant", getenv("DELIGHT"));
        const char *sfn = osl;
        float col[3] = { 0.6f, 0.7f, 0.9f };
        struct NSIParam_t esp[2] = {
            { "shaderfilename", &sfn, NSITypeString, 0, 1, 0 },
            { "i_color",        col,  NSITypeColor,  0, 1, 0 },
        };
        NSISetAttribute(ctx, "env_shader", 2, esp);
    }

    NSICreate(ctx, "drv", "outputdriver", 0, NULL);
    const char *dn = "crepro";
    const char *fn = filename;
    struct NSIParam_t dp[2] = {
        { "drivername",    &dn, NSITypeString, 0, 1, 0 },
        { "imagefilename", &fn, NSITypeString, 0, 1, 0 },
    };
    NSISetAttribute(ctx, "drv", 2, dp);
    NSIConnect(ctx, "drv", "", "layer", "outputdrivers", 0, NULL);

    (void)i;
    return ctx;
}

struct drive_args {
    NSIContext_t ctx;
    int render_ms;
    int call_end;
};

/* Drive one context start-to-teardown, matching the Rust harness's
   `drive=thread` mode: the owning thread issues Start + Synchronize, then
   Stop + Wait, then optionally NSIEnd. */
static void *drive(void *p)
{
    struct drive_args *a = (struct drive_args *)p;

    const char *start = "start";
    int one = 1;
    struct NSIParam_t start_args[3] = {
        { "action",      &start, NSITypeString,  0, 1, 0 },
        { "interactive", &one,   NSITypeInteger, 0, 1, 0 },
        { "progressive", &one,   NSITypeInteger, 0, 1, 0 },
    };
    NSIRenderControl(a->ctx, 3, start_args);

    usleep((useconds_t)a->render_ms * 1000);

    const char *sync = "synchronize", *stop = "stop", *wait = "wait";
    struct NSIParam_t act = { "action", &sync, NSITypeString, 0, 1, 0 };
    NSIRenderControl(a->ctx, 1, &act);
    act.data = &stop; NSIRenderControl(a->ctx, 1, &act);
    act.data = &wait; NSIRenderControl(a->ctx, 1, &act);

    if (a->call_end)
        NSIEnd(a->ctx);

    return NULL;
}

int main(int argc, char **argv)
{
    int delay_ms  = argc > 1 ? atoi(argv[1]) : 0;
    int count     = argc > 2 ? atoi(argv[2]) : 2;
    int render_ms = argc > 3 ? atoi(argv[3]) : 800;
    int call_end  = argc > 4 ? atoi(argv[4]) : 1;
    int threaded  = argc > 5 ? atoi(argv[5]) : 1;
    /* argv[6]: 1 = exit() straight from main (what Rust's process::exit does),
       0 = return from main normally. */
    int hard_exit = argc > 6 ? atoi(argv[6]) : 0;
    if (count > MAX_CTX) count = MAX_CTX;

    DspyRegisterDriver("crepro", crepro_open, crepro_write,
                       crepro_close, crepro_query);

    NSIContext_t ctx[MAX_CTX];
    char names[MAX_CTX][256];
    struct drive_args args[MAX_CTX];
    pthread_t th[MAX_CTX];

    for (int i = 0; i < count; ++i) {
        snprintf(names[i], sizeof names[i], "/tmp/nsi_oidn_repro_%d.exr", i);
        ctx[i] = build(i, names[i]);
        args[i].ctx = ctx[i];
        args[i].render_ms = render_ms;
        args[i].call_end = call_end;
    }

    printf("driving %d context(s), threaded=%d\n", count, threaded);
    fflush(stdout);

    if (threaded) {
        for (int i = 0; i < count; ++i)
            pthread_create(&th[i], NULL, drive, &args[i]);
        for (int i = 0; i < count; ++i)
            pthread_join(th[i], NULL);
    } else {
        for (int i = 0; i < count; ++i)
            drive(&args[i]);
    }

    printf("all contexts torn down (%d buckets)\n", g_buckets);
    printf("threads immediately after Stop+Wait+NSIEnd returned: %d\n",
           nthreads());
    for (int ms = 100; ms <= 1000; ms *= 2) {
        usleep(100 * 1000);
        printf("  threads after +%d ms: %d\n", ms, nthreads());
    }
    fflush(stdout);

    if (delay_ms > 0)
        usleep((useconds_t)delay_ms * 1000);

    if (hard_exit)
        exit(0);
    return 0;
}
