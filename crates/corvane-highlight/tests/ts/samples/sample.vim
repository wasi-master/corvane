" sample.vim - trim trailing whitespace and toggle a scratch buffer
if exists('g:loaded_sample') || &compatible
  finish
endif
let g:loaded_sample = 1
let s:save_cpo = &cpo
set cpo&vim

let g:sample_ignore = get(g:, 'sample_ignore', ['markdown', 'diff'])
let s:count = 0

" Remove trailing whitespace, keeping the cursor where it was.
function! SampleTrim(...) abort
  if index(g:sample_ignore, &filetype) >= 0
    return 0
  endif
  let l:view = winsaveview()
  let l:pattern = a:0 > 0 ? a:1 : '\s\+$'
  silent! execute '%s/' . l:pattern . '//e'
  call winrestview(l:view)
  let s:count += 1
  return 1
endfunction

function! s:ToggleScratch() abort
  let l:nr = bufnr('__scratch__')
  if l:nr != -1 && bufwinnr(l:nr) != -1
    execute bufwinnr(l:nr) . 'wincmd c'
  else
    botright new __scratch__
    setlocal buftype=nofile bufhidden=hide noswapfile
  endif
endfunction

command! -nargs=? SampleTrim call SampleTrim(<f-args>)
nnoremap <silent> <Leader>s :call <SID>ToggleScratch()<CR>

augroup sample
  autocmd!
  autocmd BufWritePre *.py,*.rs if get(b:, 'sample_trim', v:true) | call SampleTrim() | endif
augroup END

echomsg printf("sample: loaded (%d)\n", 0x10)
let &cpo = s:save_cpo
unlet s:save_cpo
