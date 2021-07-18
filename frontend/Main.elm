port module Main exposing (..)

import Browser
import Html exposing (..)
import Html.Attributes exposing (..)
import Html.Events exposing (..)
import Json.Decode as D
import Json.Encode as E
import LineChart exposing (..)
import LineChart.Area as Area
import LineChart.Axis as Axis
import LineChart.Axis.Intersection as Intersection
import LineChart.Axis.Line as AxisLine
import LineChart.Axis.Range as Range
import LineChart.Axis.Tick as Tick
import LineChart.Axis.Ticks as Ticks
import LineChart.Axis.Title as Title
import LineChart.Axis.Values as Values
import LineChart.Colors as Colors
import LineChart.Container as Container
import LineChart.Dots as Dots
import LineChart.Events as Events
import LineChart.Grid as Grid
import LineChart.Interpolation as Interpolation
import LineChart.Junk as Junk exposing (..)
import LineChart.Legends as Legends
import LineChart.Line as Line



-- MAIN


main : Program () Model Msg
main =
    Browser.element
        { init = init
        , view = view
        , update = update
        , subscriptions = subscriptions
        }



-- PORTS


port sendMessage : String -> Cmd msg


port messageReceiver : (String -> msg) -> Sub msg



-- MODEL


type Stage
    = Configuring
    | Simulating


type alias Validation =
    { value : String
    , msg : String
    }


type Field a
    = Valid a
    | Invalid Validation


type alias Config =
    { landscape : Field (List Int)
    , hours : Field Int
    }


type alias Simulation =
    { hours : Float
    , landscape : List Float
    , progress : SimulationProgress
    }


type alias SimulationProgress =
    { running : Bool
    , time : Float
    , levels : List Float
    }


type alias Model =
    { stage : Stage
    , config : Config
    , simulation : Simulation
    }


isValid : Field a -> Bool
isValid field =
    case field of
        Valid _ ->
            True

        Invalid _ ->
            False


fieldToString : (a -> String) -> Field a -> String
fieldToString toString field =
    case field of
        Invalid validation ->
            validation.value

        Valid value ->
            toString value


withLandscape : Field (List Int) -> Config -> Config
withLandscape landscape config =
    { config | landscape = landscape }


withHours : Field Int -> Config -> Config
withHours hours config =
    { config | hours = hours }


landscapeToString : Field (List Int) -> String
landscapeToString landscape =
    case landscape of
        Valid list ->
            String.join "," (List.map String.fromInt list)

        Invalid validation ->
            validation.value


parseLandscape : String -> Field (List Int)
parseLandscape value =
    let
        values =
            List.map String.trim (String.split "," value)

        segments =
            List.foldl foldSegment (Just []) values
    in
    case segments of
        Nothing ->
            Invalid { value = value, msg = "This must be a comma separated list of integers" }

        Just list ->
            Valid list


foldSegment : String -> Maybe (List Int) -> Maybe (List Int)
foldSegment value acc =
    case acc of
        Nothing ->
            Nothing

        Just list ->
            case String.toInt value of
                Nothing ->
                    Nothing

                Just int ->
                    Just (list ++ [ int ])


hoursToString : Field Int -> String
hoursToString hours =
    fieldToString String.fromInt hours


parseHours : String -> Field Int
parseHours value =
    case String.toInt value of
        Nothing ->
            Invalid { value = value, msg = "Invalid integer " ++ value }

        Just int ->
            Valid int



--- INIT


init : () -> ( Model, Cmd Msg )
init flags =
    ( { stage = Configuring
      , config =
            { landscape = Valid [ 6, 4, 5, 9, 9, 2, 6, 5, 9, 7 ]
            , hours = Valid 4
            }
      , simulation =
            { hours = 0.0
            , landscape = []
            , progress =
                { running = False
                , time = 0.0
                , levels = []
                }
            }
      }
    , Cmd.none
    )



-- UPDATE


type Msg
    = LandscapeChanged String
    | HoursChanged String
    | StartSimulation
    | PauseSimulation
    | ResumeSimulation
    | ForwardSimulation
    | Configure
    | Recv String


update : Msg -> Model -> ( Model, Cmd Msg )
update msg model =
    case msg of
        LandscapeChanged landscape ->
            ( { model | config = model.config |> withLandscape (parseLandscape landscape) }
            , Cmd.none
            )

        HoursChanged hours ->
            ( { model | config = model.config |> withHours (parseHours hours) }
            , Cmd.none
            )

        StartSimulation ->
            let
                levels =
                    case model.config.landscape of
                        Valid value ->
                            List.map Basics.toFloat value

                        Invalid _ ->
                            []

                hours =
                    case model.config.hours of
                        Valid value ->
                            Basics.toFloat value

                        Invalid _ ->
                            0.0

                simulation =
                    { hours = hours
                    , landscape = levels
                    , progress =
                        { running = True
                        , time = 0.0
                        , levels = levels
                        }
                    }
            in
            ( { model | stage = Simulating, simulation = simulation }
            , sendMessage (encodeStartSimulationEvent levels hours)
            )

        PauseSimulation ->
            let
                simulation =
                    model.simulation

                progress =
                    simulation.progress
            in
            ( model
            , sendMessage encodePauseSimulationEvent
            )

        ResumeSimulation ->
            let
                simulation =
                    model.simulation

                progress =
                    simulation.progress
            in
            ( model
            , sendMessage encodeResumeSimulationEvent
            )

        ForwardSimulation ->
            let
                simulation =
                    model.simulation

                progress =
                    simulation.progress

                finished =
                    simulation.progress.time >= simulation.hours
            in
            ( model
            , sendMessage encodeForwardSimulationEvent
            )

        Configure ->
            ( { model | stage = Configuring }
            , sendMessage encodePauseSimulationEvent
            )

        Recv event ->
            handleEvent (decodeEvent event) model



-- SUBSCRIPTIONS


subscriptions : Model -> Sub Msg
subscriptions _ =
    messageReceiver Recv



--- WS PROTOCOL


type alias ProgressParams =
    { running : Bool
    , time : Float
    , levels : List Float
    }


type Event
    = Progress ProgressParams
    | Unknown


handleEvent : Result D.Error Event -> Model -> ( Model, Cmd Msg )
handleEvent event model =
    case event of
        Ok (Progress params) ->
            handleProgress params model

        Ok Unknown ->
            ( model, Cmd.none )

        _ ->
            ( model, Cmd.none )


handleProgress : ProgressParams -> Model -> ( Model, Cmd Msg )
handleProgress params model =
    let
        simulation =
            model.simulation

        progress =
            { running = params.running, time = params.time, levels = params.levels }

        updatedSimulation =
            { simulation | progress = progress }
    in
    ( { model | simulation = updatedSimulation }
    , Cmd.none
    )


decodeEvent : String -> Result D.Error Event
decodeEvent value =
    case D.decodeString (D.field "event" D.string) value of
        Ok "progress" ->
            Result.map Progress (D.decodeString decodeProgressParams value)

        _ ->
            Result.Ok Unknown


decodeProgressParams : D.Decoder ProgressParams
decodeProgressParams =
    D.field "params"
        (D.map3 ProgressParams
            (D.field "running" D.bool)
            (D.field "time" D.float)
            (D.field "levels" (D.list D.float))
        )


encodeStartSimulationEvent : List Float -> Float -> String
encodeStartSimulationEvent landscape hours =
    E.encode 0
        (E.object
            [ ( "event", E.string "start" )
            , ( "params"
              , E.object
                    [ ( "landscape", E.list E.float landscape )
                    , ( "hours", E.float hours )
                    ]
              )
            ]
        )


encodePauseSimulationEvent : String
encodePauseSimulationEvent =
    E.encode 0 (E.object [ ( "event", E.string "pause" ) ])


encodeResumeSimulationEvent : String
encodeResumeSimulationEvent =
    E.encode 0 (E.object [ ( "event", E.string "resume" ) ])


encodeForwardSimulationEvent : String
encodeForwardSimulationEvent =
    E.encode 0 (E.object [ ( "event", E.string "forward" ) ])


encodeStopSimulationEvent : String
encodeStopSimulationEvent =
    E.encode 0 (E.object [ ( "event", E.string "stop" ) ])



-- VIEW


view : Model -> Html Msg
view model =
    let
        content =
            case model.stage of
                Configuring ->
                    viewConfigForm model.config

                Simulating ->
                    viewSimulation model.simulation
    in
    div [ class "container-sm" ] content


viewConfigForm : Config -> List (Html Msg)
viewConfigForm config =
    let
        validList =
            [ isValid config.landscape
            , isValid config.hours
            ]

        validForm =
            List.all identity validList

        buttonClass =
            if validForm then
                "btn-success"

            else
                "btn-outline-success"
    in
    [ div [ class "row" ] [ viewTitle ]
    , div [ class "row mb-4" ]
        [ div [ class "col-md-8" ] (viewLandscapeField config.landscape)
        , div [ class "col-md-4" ] (viewHoursField config.hours)
        ]
    , div [ class "row" ]
        [ div [ class "col-12" ]
            [ button
                [ class ("btn " ++ buttonClass)
                , disabled (not validForm)
                , onClick StartSimulation
                ]
                [ text "Start simulation" ]
            ]
        ]
    ]


viewTitle =
    h1 [ class "display-1 mb-4" ] [ text "Landscape Water Levels" ]


viewLandscapeField landscapeField =
    [ Html.label
        [ for "landscape"
        , class "form-label"
        ]
        [ text "Landscape" ]
    , input
        [ type_ "text"
        , class ("form-control " ++ fieldValidationClass landscapeField)
        , id "landscape"
        , onInput LandscapeChanged
        , on "keydown" (ifIsEnter StartSimulation)
        , value (landscapeToString landscapeField)
        ]
        []
    ]


viewHoursField hoursField =
    [ Html.label
        [ for "hours"
        , class "form-label"
        ]
        [ text "Hours" ]
    , input
        [ type_ "text"
        , class ("form-control " ++ fieldValidationClass hoursField)
        , id "hours"
        , onInput HoursChanged
        , on "keydown" (ifIsEnter StartSimulation)
        , value (hoursToString hoursField)
        ]
        []
    ]


fieldValidationClass : Field a -> String
fieldValidationClass field =
    case field of
        Valid a ->
            ""

        Invalid validation ->
            "is-invalid"


viewSimulation : Simulation -> List (Html Msg)
viewSimulation simulation =
    let
        len =
            List.length simulation.progress.levels

        indices =
            List.range 1 (len + 1)

        toDatum index level =
            { time = Basics.toFloat index, level = level }

        landscape_with_append =
            List.append simulation.landscape [ List.head (List.reverse simulation.landscape) |> Maybe.withDefault 0 ]

        initial =
            List.map2 toDatum indices landscape_with_append

        progress_levels_with_append =
            List.append simulation.progress.levels [ List.head (List.reverse simulation.progress.levels) |> Maybe.withDefault 0.0 ]

        levels =
            List.map2 toDatum indices progress_levels_with_append

        formatLevel value =
            let
                str =
                    String.fromFloat (toFloat (truncate (value * 10.0)) / 10.0)
            in
            if not (String.contains "." str) then
                String.concat [ str, ".0" ]

            else
                str

        levelsStr =
            let
                waterLevels =
                    List.map2 (-) simulation.progress.levels simulation.landscape
            in
                String.concat [ "[ ", String.join ", " (List.map formatLevel waterLevels), " ]" ]
    in
    [ div [ class "row" ] [ viewTitle ]
    , viewSimulationProgress simulation
    , div [ class "row" ]
        [ div [ class "col-12" ]
            [ LineChart.viewCustom (chartConfig simulation.progress.levels)
                [ LineChart.line Colors.blueLight Dots.circle "Levels" levels
                , LineChart.line Colors.rust Dots.none "Initial" initial
                ]
            ]
        ]
    , div [ class "row" ]
        [ div [ class "col-12" ]
            [ div [ class "position-absolute start-50 translate-middle-x" ]
                [ strong [ class "fs-4" ] [ text levelsStr ]
                ]
            ]
        ]
    ]


viewSimulationProgress : Simulation -> Html Msg
viewSimulationProgress simulation =
    let
        time =
            Basics.toFloat (Basics.round (simulation.progress.time * 10.0)) / 10.0

        timeLabel =
            String.fromFloat time ++ " hours"
    in
    div [ class "row my-4 gx-1 justify-content-evenly align-items-center" ]
        [ div [ class "col col-md-3 d-grid" ]
            [ div [ class "btn-group" ]
                [ button
                    [ class "btn btn-outline-secondary"
                    , attribute "data-bs-toggle" "tooltip"
                    , attribute "data-bs-placement" "top"
                    , title "Restart the current simulation"
                    , onClick StartSimulation
                    ]
                    [ Html.i [ class "bi bi-skip-start-fill" ] [] ]
                , viewPlayPauseButton simulation
                , button
                    [ class "btn btn-outline-secondary"
                    , attribute "data-bs-toggle" "tooltip"
                    , attribute "data-bs-placement" "top"
                    , title "Fast forward the current simulation"
                    , onClick ForwardSimulation
                    ]
                    [ Html.i [ class "bi bi-skip-forward-fill" ] [] ]
                , button
                    [ class "btn btn-outline-secondary"
                    , attribute "data-bs-toggle" "tooltip"
                    , attribute "data-bs-placement" "top"
                    , title "Configure another simulation"
                    , onClick Configure
                    ]
                    [ Html.i [ class "bi bi-gear-fill" ] [] ]
                ]
            ]
        , div [ class "col col-md-5 mx-4" ]
            [ viewSimulationProgressBar simulation
            ]
        , div [ class "col col-md-3" ] [ p [ class "lead fw-bolder mb-0" ] [ text timeLabel ] ]
        ]


viewPlayPauseButton simulation =
    if simulation.progress.running then
        button
            [ class "btn btn-outline-secondary"
            , attribute "data-bs-toggle" "tooltip"
            , attribute "data-bs-placement" "top"
            , title "Pause the current simulation"
            , onClick PauseSimulation
            ]
            [ Html.i [ class "bi bi-pause-fill" ] [] ]

    else
        button
            [ class "btn btn-outline-secondary"
            , attribute "data-bs-toggle" "tooltip"
            , attribute "data-bs-placement" "top"
            , title "Resume the current simulation"
            , onClick ResumeSimulation
            ]
            [ Html.i [ class "bi bi-play-fill" ] [] ]


viewSimulationProgressBar : Simulation -> Html Msg
viewSimulationProgressBar simulation =
    let
        progress =
            simulation.progress

        color =
            if (simulation.hours - progress.time) <= 0.001 then
                "bg-success"

            else
                ""

        width =
            String.fromInt (Basics.round ((progress.time / simulation.hours) * 100.0))
    in
    div [ class "progress" ]
        [ div
            [ class ("progress-bar " ++ color)
            , style "width" (width ++ "%")
            ]
            []
        ]


ifIsEnter : msg -> D.Decoder msg
ifIsEnter msg =
    D.field "key" D.string
        |> D.andThen
            (\key ->
                if key == "Enter" then
                    D.succeed msg

                else
                    D.fail "some other key"
            )



-- CHART CONFIG


type alias Datum =
    { time : Float
    , level : Float
    }


chartConfig : List Float -> LineChart.Config Datum Msg
chartConfig levels =
    { x = xAxisConfig (List.length levels + 1)
    , y = yAxisConfig (List.maximum levels |> Maybe.withDefault 10.0)
    , container = containerConfig
    , interpolation = Interpolation.stepped
    , intersection = Intersection.default
    , legends = Legends.none
    , events = Events.default
    , area = Area.normal 0.9
    , grid = Grid.default
    , line = Line.default
    , dots = Dots.default
    , junk = Junk.default
    }


xAxisConfig : Int -> Axis.Config Datum Msg
xAxisConfig length =
    Axis.custom
        { title = Title.default ""
        , variable = Just << .time
        , pixels = 1280
        , range = Range.padded 20 0
        , axisLine = AxisLine.none
        , ticks = Ticks.int length
        }


yAxisConfig : Float -> Axis.Config Datum Msg
yAxisConfig rangeSize =
    Axis.custom
        { title = Title.atAxisMax 15 -5 "Level"
        , variable = Just << .level
        , pixels = 680
        , range = Range.default
        , axisLine = yCustomAxisLine
        , ticks = yCustomTicks
        }


yCustomAxisLine : AxisLine.Config msg
yCustomAxisLine =
    AxisLine.custom <|
        \data range ->
            { color = Colors.gray
            , width = 1
            , events = []
            , start = 0
            , end = data.max
            }


yCustomTicks : Ticks.Config msg
yCustomTicks =
    Ticks.custom <|
        \data range ->
            List.map Tick.float <| Values.float (Values.around 12) { min = 0, max = data.max }


containerConfig : Container.Config Msg
containerConfig =
    Container.custom
        { attributesHtml = []
        , attributesSvg = []
        , size = Container.relative
        , margin = Container.Margin 50 60 50 75
        , id = "water-levels"
        }
