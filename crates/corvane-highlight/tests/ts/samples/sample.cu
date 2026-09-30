// SAXPY followed by a shared-memory sum reduction.
#include <cstdio>
#include <cuda_runtime.h>

#define BLOCK_SIZE 256
static bool check(cudaError_t err, const char* what) {
    if (err == cudaSuccess) return true;
    fprintf(stderr, "%s failed: %s\n", what, cudaGetErrorString(err));
    return false;
}

/* y = a * x + y, one element per thread. */
__global__ void saxpy(int n, float a, const float* __restrict__ x, float* y) {
    int i = blockIdx.x * blockDim.x + threadIdx.x;
    if (i < n) y[i] = a * x[i] + y[i];
}
__global__ void reduce_sum(const float* in, float* out, int n) {
    __shared__ float cache[BLOCK_SIZE];
    unsigned int tid = threadIdx.x;
    int i = blockIdx.x * blockDim.x + tid;
    cache[tid] = (i < n) ? in[i] : 0.0f;
    __syncthreads();
    for (unsigned int s = blockDim.x / 2; s > 0; s >>= 1) {
        if (tid < s) cache[tid] += cache[tid + s];
        __syncthreads();
    }
    if (tid == 0) atomicAdd(out, cache[0]);
}

int main() {
    const int n = 1 << 20;
    float *x = nullptr, *y = nullptr, *sum = nullptr;
    if (!check(cudaMallocManaged(&x, n * sizeof(float)), "alloc x")) return 1;
    if (!check(cudaMallocManaged(&y, n * sizeof(float)), "alloc y")) return 1;
    if (!check(cudaMallocManaged(&sum, sizeof(float)), "alloc sum")) return 1;
    for (int i = 0; i < n; ++i) { x[i] = 1.0f; y[i] = 2.0f; }
    *sum = 0.0f;
    const int blocks = (n + BLOCK_SIZE - 1) / BLOCK_SIZE;
    saxpy<<<blocks, BLOCK_SIZE>>>(n, 2.0f, x, y);
    reduce_sum<<<blocks, BLOCK_SIZE>>>(y, sum, n);
    check(cudaDeviceSynchronize(), "sync");
    printf("sum = %.1f (expected %d)\n", *sum, 4 * n);
    cudaFree(x); cudaFree(y); cudaFree(sum);
    return 0;
}
