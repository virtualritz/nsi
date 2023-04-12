#ifndef __nsi_procedural_h
#define __nsi_procedural_h

#include "nsi.h"

#ifdef  __cplusplus
extern "C" {
#endif

struct NSIProcedural_t;

/* A function that reports messages through the renderer */
typedef void (*NSIReport_t)(NSIContext_t ctx, int level, const char* message);

/* A function that cleans-up after the last execution of the procedural */
#define NSI_PROCEDURAL_UNLOAD(name) \
	void name( \
		NSIContext_t ctx, \
		NSIReport_t report, \
		struct NSIProcedural_t* proc)
typedef NSI_PROCEDURAL_UNLOAD((*NSIProceduralUnload_t));

/* A function that translates the procedural into NSI calls */
#define NSI_PROCEDURAL_EXECUTE(name) \
	void name( \
		NSIContext_t ctx, \
		NSIReport_t report, \
		struct NSIProcedural_t* proc, \
		int nparams, \
		const struct NSIParam_t* params)
typedef NSI_PROCEDURAL_EXECUTE((*NSIProceduralExecute_t));

/* Descriptor of procedural */
struct NSIProcedural_t
{
	/* Expected version of NSI */
	unsigned nsi_version;
	/* Pointers to procedural's functions */
	NSIProceduralUnload_t unload;
	NSIProceduralExecute_t execute;
};

/* Convenient macro for procedural descriptor initialization */
#define NSI_PROCEDURAL_INIT(proc, unload_fct, execute_fct) \
	{ \
		(proc).nsi_version = NSI_VERSION; \
		(proc).unload = unload_fct; \
		(proc).execute = execute_fct; \
	}

/* The entry-point of the procedural. Returns a descriptor. */
#define NSI_PROCEDURAL_LOAD_SYMBOL NSIProceduralLoad
#define NSI_PROCEDURAL_LOAD_PARAMS \
	NSIContext_t ctx, \
	NSIReport_t report, \
	const char* nsi_library_path, \
	const char* renderer_version
typedef struct NSIProcedural_t* (*NSIProceduralLoad_t)(
	NSI_PROCEDURAL_LOAD_PARAMS);

/* Convenient macro for declaration of NSIProceduralLoad */
#define NSI_PROCEDURAL_LOAD \
	_3DL_EXTERN_C _3DL_EXPORT \
		struct NSIProcedural_t* NSI_PROCEDURAL_LOAD_SYMBOL( \
			NSI_PROCEDURAL_LOAD_PARAMS)

#ifdef __cplusplus
}
#endif

#endif
