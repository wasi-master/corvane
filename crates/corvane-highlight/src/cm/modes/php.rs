//! `codemirror/mode/php/php.js`: PHP embedded in HTML.
//!
//! - `phpConfig` is a clike config (name `clike`, so PHP tokens carry
//!   `m-clike`); its hooks and the `phpString` / `matchSequence` closures run
//!   on [`clike`]'s state as `Hook::Php` and `Tokenize::Php*`, with
//!   `state.tokStack` in `State::tok_stack` and the closures' `closing` in
//!   `State::php_closing`. `text/x-php` is this config alone.
//! - The `php` mode ([`Php`], `application/x-httpd-php` and, with
//!   `startOpen`, `application/x-httpd-php-open`) switches between
//!   htmlmixed (`text/html`) and the clike config at `<?…` / `?>`, with
//!   `state.pending` finishing an html token that was cut at a `<?`.
//!
//! GHD's worker loads php.js with htmlmixed.js (and its xml, javascript and
//! css) and clike.js, so the nested htmlmixed resolves specs like
//! `text/html` does ([`super::html_modes`]).

use std::sync::{Arc, OnceLock};

use super::clike::{self, Clike, Config, Hook, State, Style, Tokenize, Words, match_consume};
use super::htmlmixed::{HtmlMixed, HtmlMixedState};
use crate::cm::{Mode, ModeState, StringStream, js_len, state, state_ref};
use crate::re;

/// `phpConfig`
fn config() -> Config {
    Config {
        keywords: Words::of(&[PHP_KEYWORDS]),
        block_keywords: Words::of(&[
            "catch do else elseif for foreach if switch try while finally",
        ]),
        def_keywords: Words::of(&["class enum function interface namespace trait"]),
        atoms: Words::of(&[PHP_ATOMS]),
        builtin: Words::of(&[PHP_BUILTIN]),
        multi_line_strings: true,
        hooks: |ch| match ch {
            '$' | '<' | '#' | '/' | '"' | '{' | '}' => Some(Hook::Php(ch)),
            _ => None,
        },
        ..Config::default()
    }
}

/// `text/x-php`: `phpConfig` as a plain clike mode
pub fn x_php() -> Arc<dyn Mode> {
    static MODE: OnceLock<Arc<dyn Mode>> = OnceLock::new();
    MODE.get_or_init(|| Arc::new(clike::with_config(config())))
        .clone()
}

/// `/[a-zA-Z_]/` then `[a-zA-Z0-9_]*`: a PHP variable after `$`
fn is_var_start(c: Option<char>) -> bool {
    c.is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
}

/// `/[\w\$_]/`
fn is_word_dollar(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || c == '$'
}

/// Loop until `?>` or the end of the line (`#` / `//` comments).
fn comment_to_close(stream: &mut StringStream) -> Style {
    while !stream.eol() && !stream.match_str("?>", false, false) {
        stream.next();
    }
    Some("comment")
}

/// `phpConfig.hooks[ch]`; `None` = the hook returned `false`.
pub(super) fn hook(ch: char, stream: &mut StringStream, state: &mut State) -> Option<Style> {
    match ch {
        '$' => {
            stream.eat_while_if(is_word_dollar);
            Some(Some("variable-2"))
        }
        '<' => {
            let before = stream.match_re(re!(r"^<<\s*"), true)?;
            let quoted = stream.eat_if(|c| c == '\'' || c == '"');
            stream.eat_while_if(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.');
            // stream.current().slice(before[0].length + (quoted ? 2 : 1))
            let from = stream.start + js_len(&before.text) + if quoted.is_some() { 2 } else { 1 };
            let delim = stream.slice(from, stream.pos).to_string();
            if let Some(q) = quoted {
                stream.eat(q);
            }
            if delim.is_empty() {
                return None;
            }
            state.tok_stack.push((delim.clone(), 0));
            state.php_closing = delim;
            state.tokenize = Tokenize::PhpString {
                escapes: quoted != Some('\''),
            };
            Some(Some("string"))
        }
        '#' => Some(comment_to_close(stream)),
        '/' => {
            stream.eat('/')?;
            Some(comment_to_close(stream))
        }
        '"' => {
            state.tok_stack.push(("\"".to_string(), 0));
            state.php_closing = "\"".to_string();
            state.tokenize = Tokenize::PhpString { escapes: true };
            Some(Some("string"))
        }
        '{' => {
            if let Some(top) = state.tok_stack.last_mut() {
                top.1 += 1;
            }
            None
        }
        '}' => {
            if let Some(top) = state.tok_stack.last_mut() {
                top.1 -= 1;
                if top.1 == 0 {
                    // phpString(state.tokStack[state.tokStack.length - 2])
                    state.php_closing = top.0.clone();
                    state.tokenize = Tokenize::PhpString { escapes: true };
                }
            }
            None
        }
        _ => None,
    }
}

/// `matchSequence(list, end, escapes)` at `step` of the `$a[…]` list
/// (`[`, number / variable-2 / variable, `]`) or the `$a->b` list (`->`,
/// variable). A finished list continues as `phpString(end)`. `escapes` is
/// never `false` here (the sequences start only when it is not).
pub(super) fn match_sequence(
    object: bool,
    step: u8,
    stream: &mut StringStream,
    state: &mut State,
) -> Style {
    let matched: Option<Style> = match (object, step) {
        (false, 0) => stream.match_str("[", true, false).then_some(None),
        (false, 1) => {
            if match_consume(stream, re!(r"^[0-9][A-Za-z0-9_.]*")) {
                Some(Some("number"))
            } else if match_consume(stream, re!(r"^\$[a-zA-Z_][a-zA-Z0-9_]*")) {
                Some(Some("variable-2"))
            } else if stream.eat_while_if(is_word_dollar) {
                Some(Some("variable"))
            } else {
                None
            }
        }
        (false, _) => stream.match_str("]", true, false).then_some(None),
        (true, 0) => stream.match_str("->", true, false).then_some(None),
        (true, _) => stream
            .eat_while_if(|c| c.is_ascii_alphanumeric() || c == '_')
            .then_some(Some("variable")),
    };
    match matched {
        Some(style) => {
            let last = if object { 1 } else { 2 };
            state.tokenize = if step >= last {
                Tokenize::PhpString { escapes: true }
            } else {
                Tokenize::PhpSequence {
                    object,
                    step: step + 1,
                }
            };
            style
        }
        None => {
            state.tokenize = Tokenize::PhpString { escapes: true };
            Some("string")
        }
    }
}

/// `phpString_(stream, state, closing, escapes)`
pub(super) fn php_string(escapes: bool, stream: &mut StringStream, state: &mut State) -> Style {
    // "Complex" syntax
    if (escapes && stream.match_str("${", false, false)) || stream.match_str("{$", false, false) {
        state.tokenize = Tokenize::Base;
        return Some("string");
    }

    // Simple syntax
    if escapes && stream.peek() == Some('$') && is_var_start(stream.char_at(stream.pos + 1)) {
        stream.next();
        stream.eat_while_if(|c| c.is_ascii_alphanumeric() || c == '_');
        // After the variable name there may appear array or object operator.
        if stream.peek() == Some('[') {
            // Match array operator
            state.tokenize = Tokenize::PhpSequence {
                object: false,
                step: 0,
            };
        }
        if stream.match_str("->", false, false)
            && stream
                .char_at(stream.pos + 2)
                .is_some_and(|c| c.is_ascii_alphanumeric() || c == '_')
        {
            // Match object operator
            state.tokenize = Tokenize::PhpSequence {
                object: true,
                step: 0,
            };
        }
        return Some("variable-2");
    }

    let mut escaped = false;
    // Normal string
    while !stream.eol()
        && (escaped
            || !escapes
            || (!stream.match_str("{$", false, false)
                // /^(\$[a-zA-Z_][a-zA-Z0-9_]*|\$\{)/
                && !(stream.peek() == Some('$')
                    && (is_var_start(stream.char_at(stream.pos + 1))
                        || stream.char_at(stream.pos + 1) == Some('{')))))
    {
        if !escaped && stream.match_str(&state.php_closing, true, false) {
            state.tokenize = Tokenize::Base;
            state.tok_stack.pop();
            break;
        }
        escaped = stream.next() == Some('\\') && !escaped;
    }
    Some("string")
}

/// `state.pending`: the rest of an html token cut at `<?`.
#[derive(Clone, Debug)]
enum Pending {
    /// a quote: finish the attribute string up to it
    Quote(char),
    /// `{end, style}`
    Token { end: usize, style: Option<String> },
}

pub struct Php {
    html: HtmlMixed,
    php: Clike,
    start_open: bool,
}

#[derive(Clone)]
pub struct PhpState {
    html: HtmlMixedState,
    /// `state.php` (`null` when closed at the top level)
    php: Option<State>,
    /// `state.curMode == phpMode`
    in_php: bool,
    pending: Option<Pending>,
}

impl Php {
    /// `CodeMirror.defineMode("php", …)`; `start_open` is
    /// `parserConfig.startOpen`.
    pub fn new(start_open: bool) -> Self {
        Self {
            html: HtmlMixed::new(Default::default(), super::html_modes),
            php: clike::with_config(config()),
            start_open,
        }
    }

    /// `dispatch(stream, state)`
    fn dispatch(&self, stream: &mut StringStream, s: &mut PhpState) -> Option<String> {
        if stream.sol() && matches!(s.pending, Some(Pending::Token { .. })) {
            s.pending = None;
        }
        if !s.in_php {
            if stream.match_re(re!(r"^<\?[A-Za-z0-9_]*"), true).is_some() {
                s.in_php = true;
                if s.php.is_none() {
                    s.php = Some(self.php.start());
                }
                return Some("meta".to_string());
            }
            let style = match s.pending.take() {
                Some(Pending::Quote(q)) => {
                    while let Some(c) = stream.next() {
                        if c == q {
                            break;
                        }
                    }
                    Some("string".to_string())
                }
                Some(Pending::Token { end, style }) if stream.pos < end => {
                    stream.pos = end;
                    style
                }
                _ => self.html.token_html_mixed(stream, &mut s.html),
            };
            let cur = stream.current();
            if let Some(open) = cur.find("<?") {
                s.pending = Some(match (style.as_deref(), cur.chars().last()) {
                    (Some("string"), Some(q @ ('\'' | '"'))) if !cur.contains("?>") => {
                        Pending::Quote(q)
                    }
                    _ => Pending::Token {
                        end: stream.pos,
                        style: style.clone(),
                    },
                });
                let back = js_len(cur) - js_len(&cur[..open]);
                stream.back_up(back);
            }
            return style;
        }
        let php = s.php.as_mut()?;
        if php.tokenize == Tokenize::Base && stream.match_str("?>", true, false) {
            s.in_php = false;
            if php.at_top_context() {
                s.php = None;
            }
            return Some("meta".to_string());
        }
        self.php.token(stream, php)
    }
}

impl Mode for Php {
    fn name(&self) -> &'static str {
        "php"
    }

    fn start_state(&self) -> Box<dyn ModeState> {
        Box::new(PhpState {
            html: self.html.start_html_mixed(),
            php: self.start_open.then(|| self.php.start()),
            in_php: self.start_open,
            pending: None,
        })
    }

    fn token(&self, stream: &mut StringStream, st: &mut dyn ModeState) -> Option<String> {
        self.dispatch(stream, state::<PhpState>(st))
    }

    /// `innerMode`: `{state: state.curState, mode: state.curMode}`
    fn inner_mode_name(&self, st: &dyn ModeState) -> &'static str {
        let s = state_ref::<PhpState>(st);
        if s.in_php {
            "clike"
        } else {
            self.html.inner_name(&s.html)
        }
    }
}

/// `application/x-httpd-php`
pub fn php() -> Arc<dyn Mode> {
    static MODE: OnceLock<Arc<dyn Mode>> = OnceLock::new();
    MODE.get_or_init(|| Arc::new(Php::new(false))).clone()
}

/// `application/x-httpd-php-open` (`{name: "php", startOpen: true}`)
pub fn php_open() -> Arc<dyn Mode> {
    static MODE: OnceLock<Arc<dyn Mode>> = OnceLock::new();
    MODE.get_or_init(|| Arc::new(Php::new(true))).clone()
}

/// `phpKeywords`
const PHP_KEYWORDS: &str = "abstract and array as break case catch class clone const continue declare default do else \
elseif enddeclare endfor endforeach endif endswitch endwhile enum extends final for \
foreach function global goto if implements interface instanceof namespace new or private \
protected public static switch throw trait try use var while xor die echo empty exit eval \
include include_once isset list require require_once return print unset __halt_compiler \
self static parent yield insteadof finally readonly match";

/// `phpAtoms`
const PHP_ATOMS: &str = "true false null TRUE FALSE NULL __CLASS__ __DIR__ __FILE__ __LINE__ __METHOD__ \
__FUNCTION__ __NAMESPACE__ __TRAIT__";

/// `phpBuiltin`
const PHP_BUILTIN: &str = "func_num_args func_get_arg func_get_args strlen strcmp strncmp strcasecmp strncasecmp each \
error_reporting define defined trigger_error user_error set_error_handler \
restore_error_handler get_declared_classes get_loaded_extensions extension_loaded \
get_extension_funcs debug_backtrace constant bin2hex hex2bin sleep usleep time mktime \
gmmktime strftime gmstrftime strtotime date gmdate getdate localtime checkdate flush \
wordwrap htmlspecialchars htmlentities html_entity_decode md5 md5_file crc32 getimagesize \
image_type_to_mime_type phpinfo phpversion phpcredits strnatcmp strnatcasecmp substr_count \
strspn strcspn strtok strtoupper strtolower strpos strrpos strrev hebrev hebrevc nl2br \
basename dirname pathinfo stripslashes stripcslashes strstr stristr strrchr str_shuffle \
str_word_count strcoll substr substr_replace quotemeta ucfirst ucwords strtr addslashes \
addcslashes rtrim str_replace str_repeat count_chars chunk_split trim ltrim strip_tags \
similar_text explode implode setlocale localeconv parse_str str_pad chop strchr sprintf \
printf vprintf vsprintf sscanf fscanf parse_url urlencode urldecode rawurlencode \
rawurldecode readlink linkinfo link unlink exec system escapeshellcmd escapeshellarg \
passthru shell_exec proc_open proc_close rand srand getrandmax mt_rand mt_srand \
mt_getrandmax base64_decode base64_encode abs ceil floor round is_finite is_nan \
is_infinite bindec hexdec octdec decbin decoct dechex base_convert number_format fmod \
ip2long long2ip getenv putenv getopt microtime gettimeofday getrusage uniqid \
quoted_printable_decode set_time_limit get_cfg_var magic_quotes_runtime \
set_magic_quotes_runtime get_magic_quotes_gpc get_magic_quotes_runtime \
import_request_variables error_log serialize unserialize memory_get_usage \
memory_get_peak_usage var_dump var_export debug_zval_dump print_r highlight_file \
show_source highlight_string ini_get ini_get_all ini_set ini_alter ini_restore \
get_include_path set_include_path restore_include_path setcookie header headers_sent \
connection_aborted connection_status ignore_user_abort parse_ini_file is_uploaded_file \
move_uploaded_file intval floatval doubleval strval gettype settype is_null is_resource \
is_bool is_long is_float is_int is_integer is_double is_real is_numeric is_string is_array \
is_object is_scalar ereg ereg_replace eregi eregi_replace split spliti join sql_regcase dl \
pclose popen readfile rewind rmdir umask fclose feof fgetc fgets fgetss fread fopen \
fpassthru ftruncate fstat fseek ftell fflush fwrite fputs mkdir rename copy tempnam \
tmpfile file file_get_contents file_put_contents stream_select stream_context_create \
stream_context_set_params stream_context_set_option stream_context_get_options \
stream_filter_prepend stream_filter_append fgetcsv flock get_meta_tags \
stream_set_write_buffer set_file_buffer set_socket_blocking stream_set_blocking \
socket_set_blocking stream_get_meta_data stream_register_wrapper stream_wrapper_register \
stream_set_timeout socket_set_timeout socket_get_status realpath fnmatch fsockopen \
pfsockopen pack unpack get_browser crypt opendir closedir chdir getcwd rewinddir readdir \
dir glob fileatime filectime filegroup fileinode filemtime fileowner fileperms filesize \
filetype file_exists is_writable is_writeable is_readable is_executable is_file is_dir \
is_link stat lstat chown touch clearstatcache mail ob_start ob_flush ob_clean ob_end_flush \
ob_end_clean ob_get_flush ob_get_clean ob_get_length ob_get_level ob_get_status \
ob_get_contents ob_implicit_flush ob_list_handlers ksort krsort natsort natcasesort asort \
arsort sort rsort usort uasort uksort shuffle array_walk count end prev next reset current \
key min max in_array array_search extract compact array_fill range array_multisort \
array_push array_pop array_shift array_unshift array_splice array_slice array_merge \
array_merge_recursive array_keys array_values array_count_values array_reverse \
array_reduce array_pad array_flip array_change_key_case array_rand array_unique \
array_intersect array_intersect_assoc array_diff array_diff_assoc array_sum array_filter \
array_map array_chunk array_key_exists array_intersect_key array_combine array_column pos \
sizeof key_exists assert assert_options version_compare ftok str_rot13 aggregate \
session_name session_module_name session_save_path session_id session_regenerate_id \
session_decode session_register session_unregister session_is_registered session_encode \
session_start session_destroy session_unset session_set_save_handler session_cache_limiter \
session_cache_expire session_set_cookie_params session_get_cookie_params \
session_write_close preg_match preg_match_all preg_replace preg_replace_callback \
preg_split preg_quote preg_grep overload ctype_alnum ctype_alpha ctype_cntrl ctype_digit \
ctype_lower ctype_graph ctype_print ctype_punct ctype_space ctype_upper ctype_xdigit \
virtual apache_request_headers apache_note apache_lookup_uri apache_child_terminate \
apache_setenv apache_response_headers apache_get_version getallheaders mysql_connect \
mysql_pconnect mysql_close mysql_select_db mysql_create_db mysql_drop_db mysql_query \
mysql_unbuffered_query mysql_db_query mysql_list_dbs mysql_list_tables mysql_list_fields \
mysql_list_processes mysql_error mysql_errno mysql_affected_rows mysql_insert_id \
mysql_result mysql_num_rows mysql_num_fields mysql_fetch_row mysql_fetch_array \
mysql_fetch_assoc mysql_fetch_object mysql_data_seek mysql_fetch_lengths mysql_fetch_field \
mysql_field_seek mysql_free_result mysql_field_name mysql_field_table mysql_field_len \
mysql_field_type mysql_field_flags mysql_escape_string mysql_real_escape_string mysql_stat \
mysql_thread_id mysql_client_encoding mysql_get_client_info mysql_get_host_info \
mysql_get_proto_info mysql_get_server_info mysql_info mysql mysql_fieldname \
mysql_fieldtable mysql_fieldlen mysql_fieldtype mysql_fieldflags mysql_selectdb \
mysql_createdb mysql_dropdb mysql_freeresult mysql_numfields mysql_numrows mysql_listdbs \
mysql_listtables mysql_listfields mysql_db_name mysql_dbname mysql_tablename \
mysql_table_name pg_connect pg_pconnect pg_close pg_connection_status pg_connection_busy \
pg_connection_reset pg_host pg_dbname pg_port pg_tty pg_options pg_ping pg_query \
pg_send_query pg_cancel_query pg_fetch_result pg_fetch_row pg_fetch_assoc pg_fetch_array \
pg_fetch_object pg_fetch_all pg_affected_rows pg_get_result pg_result_seek \
pg_result_status pg_free_result pg_last_oid pg_num_rows pg_num_fields pg_field_name \
pg_field_num pg_field_size pg_field_type pg_field_prtlen pg_field_is_null pg_get_notify \
pg_get_pid pg_result_error pg_last_error pg_last_notice pg_put_line pg_end_copy pg_copy_to \
pg_copy_from pg_trace pg_untrace pg_lo_create pg_lo_unlink pg_lo_open pg_lo_close \
pg_lo_read pg_lo_write pg_lo_read_all pg_lo_import pg_lo_export pg_lo_seek pg_lo_tell \
pg_escape_string pg_escape_bytea pg_unescape_bytea pg_client_encoding \
pg_set_client_encoding pg_meta_data pg_convert pg_insert pg_update pg_delete pg_select \
pg_exec pg_getlastoid pg_cmdtuples pg_errormessage pg_numrows pg_numfields pg_fieldname \
pg_fieldsize pg_fieldtype pg_fieldnum pg_fieldprtlen pg_fieldisnull pg_freeresult \
pg_result pg_loreadall pg_locreate pg_lounlink pg_loopen pg_loclose pg_loread pg_lowrite \
pg_loimport pg_loexport http_response_code get_declared_traits getimagesizefromstring \
socket_import_stream stream_set_chunk_size trait_exists header_register_callback \
class_uses session_status session_register_shutdown echo print global static exit array \
empty eval isset unset die include require include_once require_once json_decode \
json_encode json_last_error json_last_error_msg curl_close curl_copy_handle curl_errno \
curl_error curl_escape curl_exec curl_file_create curl_getinfo curl_init \
curl_multi_add_handle curl_multi_close curl_multi_exec curl_multi_getcontent \
curl_multi_info_read curl_multi_init curl_multi_remove_handle curl_multi_select \
curl_multi_setopt curl_multi_strerror curl_pause curl_reset curl_setopt_array curl_setopt \
curl_share_close curl_share_init curl_share_setopt curl_strerror curl_unescape \
curl_version mysqli_affected_rows mysqli_autocommit mysqli_change_user \
mysqli_character_set_name mysqli_close mysqli_commit mysqli_connect_errno \
mysqli_connect_error mysqli_connect mysqli_data_seek mysqli_debug mysqli_dump_debug_info \
mysqli_errno mysqli_error_list mysqli_error mysqli_fetch_all mysqli_fetch_array \
mysqli_fetch_assoc mysqli_fetch_field_direct mysqli_fetch_field mysqli_fetch_fields \
mysqli_fetch_lengths mysqli_fetch_object mysqli_fetch_row mysqli_field_count \
mysqli_field_seek mysqli_field_tell mysqli_free_result mysqli_get_charset \
mysqli_get_client_info mysqli_get_client_stats mysqli_get_client_version \
mysqli_get_connection_stats mysqli_get_host_info mysqli_get_proto_info \
mysqli_get_server_info mysqli_get_server_version mysqli_info mysqli_init mysqli_insert_id \
mysqli_kill mysqli_more_results mysqli_multi_query mysqli_next_result mysqli_num_fields \
mysqli_num_rows mysqli_options mysqli_ping mysqli_prepare mysqli_query mysqli_real_connect \
mysqli_real_escape_string mysqli_real_query mysqli_reap_async_query mysqli_refresh \
mysqli_rollback mysqli_select_db mysqli_set_charset mysqli_set_local_infile_default \
mysqli_set_local_infile_handler mysqli_sqlstate mysqli_ssl_set mysqli_stat \
mysqli_stmt_init mysqli_store_result mysqli_thread_id mysqli_thread_safe mysqli_use_result \
mysqli_warning_count";
