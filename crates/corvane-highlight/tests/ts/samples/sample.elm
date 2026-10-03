module Sample exposing (Model, Msg(..), main)

{-| A counter that remembers its history. -}

import Browser
import Html exposing (Html, button, div, li, text, ul)
import Html.Events exposing (onClick)

-- MODEL
type alias Model =
    { count : Int, history : List String, step : Float }

type Msg
    = Increment
    | Decrement
    | Reset

init : Model
init =
    { count = 0, history = [], step = 1.5 }

update : Msg -> Model -> Model
update msg model =
    case msg of
        Increment ->
            { model | count = model.count + 1, history = "inc" :: model.history }
        Decrement ->
            if model.count > 0 then { model | count = model.count - 1 } else model
        Reset ->
            init

view : Model -> Html Msg
view model =
    let
        label = "Count: " ++ String.fromInt model.count ++ "\n"
    in
    div []
        [ button [ onClick Decrement ] [ text "-" ]
        , text label
        , ul [] (List.map (\h -> li [] [ text h ]) model.history)
        , button [ onClick Increment ] [ text "+" ]
        ]
main : Program () Model Msg
main =
    Browser.sandbox { init = init, update = update, view = view }
