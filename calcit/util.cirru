
{} (:about "|file is generated - never edit directly; learn cr edit/tree workflows before changing") (:package |util)
  :configs $ {} (:init-fn |util.core/main!) (:reload-fn |util.core/reload!) (:version |0.0.0)
    :modules $ []
  :entries $ {}
  :files $ {}
    |util.core $ %{} :FileEntry
      :defs $ {}
        |inside-eval: $ %{} :CodeEntry (:doc |)
          :code $ quote
            defmacro inside-eval: (& body)
              if
                = :eval $ &get-calcit-running-mode
                quasiquote $ do (println "|env: eval") ~@body
                quasiquote $ do (println "|env: not eval. tests skipped")
          :examples $ []
          :schema $ quote
            [] $ {} (:kind :macro)
              :args $ [] :dynamic
              :return :dynamic
        |inside-js: $ %{} :CodeEntry (:doc |)
          :code $ quote
            defmacro inside-js: (& body)
              if
                not= :eval $ &get-calcit-running-mode
                quasiquote $ do (println "|env: js") ~@body
                quasiquote $ do (println "|env: not js. tests skipped")
          :examples $ []
          :schema $ quote
            [] $ {} (:kind :macro)
              :args $ [] :dynamic
              :return :dynamic
        |log-title $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn log-title (title) (println) (println title) (println)
          :examples $ []
          :schema $ quote
            [] $ {} (:kind :fn)
              :args $ [] :dynamic
              :return :dynamic
        |main! $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn main! () $ :: :unit
          :examples $ []
          :schema $ quote
            [] $ {} (:kind :fn)
              :args $ []
              :return :dynamic
        |reload! $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn reload! () $ :: :unit
          :examples $ []
          :schema $ quote
            [] $ {} (:kind :fn)
              :args $ []
              :return :dynamic
      :ns $ %{} :CodeEntry (:doc |) (:schema nil)
        :code $ quote
          ns util.core $ :require
        :examples $ []
