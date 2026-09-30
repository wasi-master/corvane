%%% @doc A tiny key-value store process.
-module(sample).
-behaviour(gen_server).

-export([start_link/0, store/2, fetch/1]).
-export([init/1, handle_call/3, handle_cast/2]).

-define(SERVER, ?MODULE).
-define(TIMEOUT, 5000).

-record(state, {table = #{} :: map(), hits = 0 :: non_neg_integer()}).

-type key() :: atom() | binary().

%% Public API
-spec start_link() -> {ok, pid()} | {error, term()}.
start_link() ->
    gen_server:start_link({local, ?SERVER}, ?MODULE, [], []).

-spec store(key(), term()) -> ok.
store(Key, Value) ->
    gen_server:cast(?SERVER, {put, Key, Value}).

fetch(Key) ->
    gen_server:call(?SERVER, {get, Key}, ?TIMEOUT).

%% Callbacks
init([]) ->
    {ok, #state{}}.

handle_call({get, Key}, _From, #state{table = T, hits = H} = S) ->
    Reply = case maps:find(Key, T) of
                {ok, V} -> {ok, V};
                error when is_atom(Key) -> {error, not_found};
                error -> {error, <<"bad key">>}
            end,
    {reply, Reply, S#state{hits = H + 1}};
handle_call(_Other, _From, S) ->
    {reply, {error, unknown}, S}.

handle_cast({put, Key, Value}, S = #state{table = T}) ->
    io:format("put ~p -> ~p~n", [Key, Value]),
    {noreply, S#state{table = T#{Key => Value}}}.
