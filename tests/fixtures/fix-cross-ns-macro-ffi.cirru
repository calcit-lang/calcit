{}
  :package |defprobe
  :entries $ {} $ :default
    {} (:description "|probe") (:init-fn 'defprobe.main/main!) (:mode :native) (:reload-fn 'defprobe.main/reload!)
      :feature-policy $ {} (:js-ffi :error)
      :modules $ []
      :type-slots $ {}
  :files $ {}
    'defprobe.macros $ %{} 'FileEntry
      :defs $ {}
        'Effect $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct Effect (:name 'Tag) (:coord 'List) (:args 'List) (:method 'Fn)
          :examples $ []
          :schema $ :: 'StructDef
        'DomElement $ %{} 'CodeEntry (:doc |)
          :code $ quote $ deftrait DomElement
            .select! $ :: 'Fn $ {} (:args $ [] 'defprobe.macros/DomElement) (:return 'Unit)
          :examples $ []
          :ffi $ {} (:backend :js) (:kind :external-object)
            :names $ {} (:select! |select)
          :schema $ :: 'Trait
        'defeffect $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defmacro defeffect (effect-name args params & body)
            let
                args-var $ gensym |args
                params-var $ gensym |params
              quasiquote $ defn ~effect-name ~args $ %{} defprobe.macros/Effect
                :name $ turn-tag effect-name
                :coord $ []
                :args $ [] ~@args
                :method $ fn (~args-var ~params-var)
                  let[] ~args ~args-var $ let[] ~params ~params-var $ ~
                    quasiquote $ do ~@body
          :examples $ []
          :schema $ :: 'Macro $ {}
            :capabilities $ #{}
            :expansion $ :: 'Definition 'Fn
            :rest 'Syntax
            :required $ [] 'SyntaxSymbol 'SyntaxList 'SyntaxList
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns defprobe.macros
    'defprobe.main $ %{} 'FileEntry
      :defs $ {}
        'effect-focus $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defprobe.macros/defeffect effect-focus (pattern) (action el at-place?)
            when (= action :mount)
              .select! $ unsafe-coerce el 'defprobe.macros/DomElement
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'defprobe.macros/Effect)
            :args $ [] 'Dynamic
            :features $ #{} :js-ffi
        'main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main! ()
            hint-fn $ {} (:args $ []) (:return 'Unit)
            , &unit
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit) (:args $ [])
        'reload! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn reload! ()
            hint-fn $ {} (:args $ []) (:return 'Unit)
            , &unit
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit) (:args $ [])
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns defprobe.main
          :require $ defprobe.macros :refer $ defeffect
