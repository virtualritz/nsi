#ifndef __nsi_h
#define __nsi_h

#include <stddef.h>

#ifdef _WIN32
    #define DL_INTERFACE __declspec(dllimport)
#else
    #define DL_INTERFACE
#endif

#ifdef  __cplusplus
extern "C" {
#endif

typedef int NSIContext_t;
typedef const char* NSIHandle_t;

#define NSI_BAD_CONTEXT ((NSIContext_t)0)
#define NSI_SCENE_ROOT ".root"
#define NSI_SCENE_GLOBAL ".global"
#define NSI_ALL_NODES ".all"
#define NSI_ALL_ATTRIBUTES ".all"
#define NSI_VERSION 2

/* Type values for NSIParam_t.type */
enum NSIType_t
{
	NSITypeInvalid = 0,
	NSITypeFloat = 1,
	NSITypeDouble = NSITypeFloat | 0x10,
	NSITypeInteger = 2,
	NSITypeString = 3,
	NSITypeColor = 4,
	NSITypePoint = 5,
	NSITypeVector = 6,
	NSITypeNormal = 7,
	NSITypeMatrix = 8,
	NSITypeDoubleMatrix = NSITypeMatrix | 0x10,
	NSITypePointer = 9
};

static inline
size_t NSITypeSizeOf(unsigned t)
{
	static const unsigned char sizes[] =
	{
		0, sizeof(float), sizeof(int), sizeof(char*),
		3*sizeof(float), 3*sizeof(float), 3*sizeof(float), 3*sizeof(float),
		16*sizeof(float), sizeof(void*), 0, 0,
		0, 0, 0, 0,
		0, sizeof(double), 0, 0,
		0, 0, 0, 0,
		16*sizeof(double)
	};
	return t <= 24 ? sizes[t] : 0;
}

/* Flag values for NSIParam_t.flags */
enum
{
	NSIParamIsArray = 1,
	NSIParamPerFace = 2,
	NSIParamPerVertex = 4,
	NSIParamInterpolateLinear = 8
};

/* Structure for optional parameters. */
struct NSIParam_t
{
	const char *name;
	const void *data;
	int type;
	int arraylength;
	size_t count;
	int flags;
};

/* Values for second parameter of NSIRenderStopped_t */
enum NSIStoppingStatus
{
	NSIRenderCompleted = 0,
	NSIRenderAborted = 1,
	NSIRenderSynchronized = 2,
	NSIRenderRestarted = 3
};

/* Error levels for the error callback. */
enum NSIErrorLevel
{
	NSIErrMessage = 0,
	NSIErrInfo = 1,
	NSIErrWarning = 2,
	NSIErrError = 3
};

/* Error handler callback type. */
typedef void (*NSIErrorHandler_t)(
	void *userdata, int level, int code, const char *message );

/* Stopped callback type. */
typedef void (*NSIRenderStopped_t)(
	void *userdata, NSIContext_t ctx, int status );

DL_INTERFACE NSIContext_t NSIBegin(
	int nparams,
	const struct NSIParam_t *params );

DL_INTERFACE void NSIEnd( NSIContext_t ctx );

DL_INTERFACE void NSICreate(
	NSIContext_t ctx,
	NSIHandle_t handle,
	const char *type,
	int nparams,
	const struct NSIParam_t *params );

DL_INTERFACE void NSIDelete(
	NSIContext_t ctx,
	NSIHandle_t handle,
	int nparams,
	const struct NSIParam_t *params );

DL_INTERFACE void NSISetAttribute(
	NSIContext_t ctx,
	NSIHandle_t object,
	int nparams,
	const struct NSIParam_t *params );

DL_INTERFACE void NSISetAttributeAtTime(
	NSIContext_t ctx,
	NSIHandle_t object,
	double time,
	int nparams,
	const struct NSIParam_t *params );

DL_INTERFACE void NSIDeleteAttribute(
	NSIContext_t ctx,
	NSIHandle_t object,
	const char *name );

DL_INTERFACE void NSIConnect(
	NSIContext_t ctx,
	NSIHandle_t from,
	const char *from_attr,
	NSIHandle_t to,
	const char *to_attr,
	int nparams,
	const struct NSIParam_t *params );

DL_INTERFACE void NSIDisconnect(
	NSIContext_t ctx,
	NSIHandle_t from,
	const char *from_attr,
	NSIHandle_t to,
	const char *to_attr );

DL_INTERFACE void NSIEvaluate(
	NSIContext_t ctx,
	int nparams,
	const struct NSIParam_t *params );

DL_INTERFACE void NSIRenderControl(
	NSIContext_t ctx,
	int nparams,
	const struct NSIParam_t *params );

#ifdef __cplusplus
}
#endif

#endif
