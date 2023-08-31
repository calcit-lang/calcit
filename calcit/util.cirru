
{} (:package |util)
  :configs $ {} (:init-fn |util.core/main!) (:reload-fn |util.core/reload!)
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
        |inside-js: $ %{} :CodeEntry (:doc |)
          :code $ quote
            defmacro inside-js: (& body)
              if
                not= :eval $ &get-calcit-running-mode
                quasiquote $ do (println "|env: js") ~@body
                quasiquote $ do (println "|env: not js. tests skipped")
        |log-title $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn log-title (title) (println) (println title) (println)
        |main! $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn main! () $ :: :unit
        |reload! $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn reload! () $ :: :unit
      :ns $ %{} :CodeEntry (:doc |)
        :code $ quote
          ns util.core $ :require
