import { HttpErrorResponse, HttpInterceptorFn } from '@angular/common/http';
import { inject } from '@angular/core';
import { Router } from '@angular/router';
import { catchError, throwError } from 'rxjs';

import { Problem } from '../api/models/problem';
import { Session } from './auth';

/** Une session expirée renvoie à l'écran de connexion (ADR-0015). */
export const sessionInterceptor: HttpInterceptorFn = (request, next) => {
  const router = inject(Router);
  const session = inject(Session);
  return next(request).pipe(
    catchError((error: unknown) => {
      const isAuthCall = request.url.includes('/api/v1/auth/');
      if (error instanceof HttpErrorResponse && error.status === 401 && !isAuthCall) {
        session.markLoggedOut();
        void router.navigate(['/connexion'], { queryParams: { retour: router.url } });
      }
      return throwError(() => error);
    }),
  );
};

/** Corps `application/problem+json` d'une erreur de l'API, s'il y en a un (ADR-0016). */
export function problemOf(error: unknown): Problem | null {
  if (!(error instanceof HttpErrorResponse) || !error.error) {
    return null;
  }
  // Les routes sans corps de réponse (204) sont lues en texte : leur erreur arrive alors en chaîne.
  let body: unknown = error.error;
  if (typeof body === 'string') {
    try {
      body = JSON.parse(body);
    } catch {
      return null;
    }
  }
  return body && typeof body === 'object' && 'code' in body ? (body as Problem) : null;
}

/** Message à afficher pour une erreur (le code de règle n'est pas montré, ecrans.md). */
export function errorMessage(error: unknown): string {
  const problem = problemOf(error);
  if (problem) {
    return problem.title;
  }
  if (error instanceof HttpErrorResponse && error.status === 0) {
    return 'Serveur injoignable.';
  }
  return 'Une erreur est survenue.';
}
